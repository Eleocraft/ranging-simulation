//! Distributed weighted multidimensional scaling (dwMDS).
//!
//! Decentralized node localization following Costa, Patwari and Hero,
//! *"Distributed Weighted-Multidimensional Scaling for Node Localization in
//! Sensor Networks"*.
//!
//! Every node owns one [`DwMDS`] instance and only ever sees its own
//! neighbourhood: the ranges it measured and the position estimates its
//! neighbours broadcast. Indexing the `n` blindfolded nodes `1..=n` and the
//! `m` anchors `n+1..=n+m`, the global stress (paper eq. 4) is
//!
//! ```text
//! S = 2 sum_{i <= n} sum_{i < j <= n+m} w_ij (delta_ij - ||x_i - x_j||)^2
//!   + sum_{i <= n} r_i ||x_i - xbar_i||^2
//! ```
//!
//! and it splits exactly into the per-node local costs [`DwMDS::cost`],
//! `S = sum_i S_i + c` (eq. 5), so a node can improve the global solution
//! using local data alone. One communication round is
//!
//! 1. range against the neighbours and record them with [`DwMDS::observe`]
//!    or [`DwMDS::observe_anchor`],
//! 2. [`DwMDS::solve`] to re-minimize the local cost,
//! 3. broadcast [`DwMDS::position`] so the neighbours can do the same.
//!
//! The link weights `w_ij` are supplied by the caller, derived from the RF
//! quality indicators of the ranging exchange; see [`DwMDS::observe`] for what
//! the algorithm expects of them.
//!
//! Anchored links enter the local cost at twice the weight, see [`DwMDS::observe_anchor`].

extern crate alloc;
use alloc::collections::BTreeMap;

use nalgebra as na;

/// Tuning parameters of the local solver.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Relative cost decrease below which [`DwMDS::solve`] stops iterating.
    pub tolerance: f32,
    /// Iteration cap for a single [`DwMDS::solve`] call.
    pub max_iterations: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            tolerance: 1e-4,
            max_iterations: 50,
        }
    }
}

struct Neighbour {
    /// Position estimate this neighbour broadcast last.
    x_j: na::Vector3<f32>,
    /// Weight `w_ij` of the link.
    w_ij: f32,
    /// Measured range `delta_ij`.
    delta_ij: f32,
    /// Whether the neighbour is an anchor, i.e. knows its own position and
    /// runs no dwMDS of its own.
    anchor: bool,
}

impl Neighbour {
    /// Weight of this link as it enters the local cost (paper eq. 6).
    ///
    /// The global stress assigns `2 w_ij` to every pair. A link between two
    /// blindfolded nodes appears in two local costs, `S_i` and `S_j`, so each
    /// end carries `w_ij` and the halves add back up. An anchor runs no local
    /// cost to carry its half, so `S_i` has to hold the full `2 w_ij`.
    fn effective_weight(&self) -> f32 {
        if self.anchor { 2. * self.w_ij } else { self.w_ij }
    }
}

pub struct DwMDS {
    /// Own position estimate `x_i`.
    x_i: na::Vector3<f32>,
    /// Neighbourhood state, keyed by node id.
    x: BTreeMap<u32, Neighbour>,
    /// Prior estimate `xbar_i` the regularization term pulls towards.
    x_bar: na::Vector3<f32>,
    /// Regularization weight `r_i`.
    r_i: f32,
    /// Static algorithm configuration values
    config: Config,
}

impl DwMDS {
    /// Starts from an initial guess, without a prior (`r_i = 0`).
    pub fn new(initial: na::Vector3<f32>, config: Config) -> Self {
        Self {
            x_i: initial,
            x: BTreeMap::new(),
            x_bar: na::Vector3::zeros(),
            r_i: 0.,
            config,
        }
    }

    /// Starts at zero without additional information.
    /// **solving from unseeded nodes will cause the algorithm to deadlock.**
    /// call `seed_from_neighbours()` after observing a sufficient number of neighbours
    /// in order to keep that from happening.
    pub fn unseeded(config: Config) -> Self {
        Self {
            x_i: na::Vector3::zeros(),
            x: BTreeMap::new(),
            x_bar: na::Vector3::zeros(),
            r_i: 0.,
            config,
        }
    }

    /// Starts from a prior position, weighted with `weight`.
    pub fn with_prior(prior: na::Vector3<f32>, weight: f32, config: Config) -> Self {
        Self {
            x_i: prior,
            x: BTreeMap::new(),
            x_bar: prior,
            r_i: weight,
            config,
        }
    }

    /// Current position estimate, the value to broadcast to the neighbours.
    pub fn position(&self) -> na::Vector3<f32> {
        self.x_i
    }

    /// Overrides the prior and its weight, e.g. when odometry or a previous
    /// localization round provides a new reference.
    pub fn set_prior(&mut self, prior: na::Vector3<f32>, weight: f32) {
        self.x_bar = prior;
        self.r_i = weight;
    }

    /// Records a ranging exchange: the position `pos` the neighbour broadcast,
    /// the measured range `range` to it, and the confidence `weight` in that
    /// range, derived from the RF quality indicators of the exchange.
    ///
    /// Repeated exchanges with the same neighbour **accumulate**:
    /// the weights add and the ranges average by weight,
    ///
    /// ```text
    /// w_ij    <- w_ij + w_new
    /// delta_ij <- (w_ij delta_ij + w_new range) / (w_ij + w_new)
    /// ```
    ///
    /// This is exact, not an approximation — expanding the squares shows the
    /// `K` separate cost terms equal one term in the aggregate `(w, delta)`
    /// plus a constant. Ten consistent exchanges therefore pull ten times as
    /// hard as one, which is the intended behaviour for a static network. A
    /// node that is physically moved must be dropped with [`DwMDS::forget`]
    /// first, otherwise its accumulated history keeps voting for where it
    /// used to be.
    ///
    /// A `weight` of zero or less records no link: the paper reads `w_ij = 0`
    /// as "`j` is not in the neighbourhood of `i`" (eq. 16), so no entry is
    /// created and an existing one keeps everything it had accumulated. The
    /// broadcast `pos` is still taken, since it does not come from the range.
    ///
    /// Two properties matter for the distributed algorithm to behave:
    ///
    /// - **Symmetry.** Node `j` must weight this link the same way, because
    ///   the global stress counts the pair once and both local costs have to
    ///   agree on it. Derive the weight from quantities both ends observe.
    /// - **Inverse variance.** The paper reads `w_ij` as the precision
    ///   `1/(2 sigma^2)` of the range estimate. Calibrating the RF quality
    ///   mapping that way makes the accumulation above the statistically
    ///   correct fusion of independent measurements, and fixes the scale
    ///   against `r_i`, which is *not* rescaled along with the weights.
    ///
    /// Use [`DwMDS::observe_anchor`] instead when the neighbour is an anchor.
    pub fn observe(&mut self, id: u32, pos: na::Vector3<f32>, range: f32, weight: f32) {
        self.insert(id, pos, range, weight, false);
    }

    /// Like [`DwMDS::observe`], for a neighbour that knows its own position
    /// and runs no dwMDS of its own.
    ///
    /// The link is weighted `2 w_ij` rather than `w_ij` (paper eq. 6): a peer
    /// link is shared between `S_i` and `S_j`, an anchor link is carried by
    /// `S_i` alone. Passing an anchor to [`DwMDS::observe`] silently halves
    /// its influence relative to the peers in the same neighbourhood.
    pub fn observe_anchor(&mut self, id: u32, pos: na::Vector3<f32>, range: f32, weight: f32) {
        self.insert(id, pos, range, weight, true);
    }

    fn insert(&mut self, id: u32, pos: na::Vector3<f32>, range: f32, weight: f32, anchor: bool) {
        // A weightless exchange carries no range evidence, so nothing
        // accumulates. The position the neighbour broadcast is independent of
        // the range and still current, so it is worth keeping. An unknown
        // neighbour stays unknown: zero weight means "not in the
        // neighbourhood" (paper eq. 16).
        if weight <= 0. {
            self.set_neighbour_position(id, pos);
            return;
        }

        // if the node alredy exists in x, sum up weight and update range
        if let Some(n) = self.x.get_mut(&id)
            && n.anchor == anchor
        {
            let w_ij = n.w_ij + weight;
            n.delta_ij = (n.w_ij * n.delta_ij + weight * range) / w_ij;
            n.w_ij = w_ij;
            n.x_j = pos;
            return;
        }

        // A neighbour that changed kind is a different node as far as the cost
        // is concerned, so its accumulator restarts.
        self.x.insert(
            id,
            Neighbour {
                x_j: pos,
                w_ij: weight,
                delta_ij: range,
                anchor,
            },
        );
    }

    /// Updates the cached estimate of a neighbour that re-broadcast its
    /// position without a new ranging exchange. Unknown ids are ignored, as
    /// the link has no measured range yet.
    pub fn set_neighbour_position(&mut self, id: u32, pos: na::Vector3<f32>) {
        if let Some(n) = self.x.get_mut(&id) {
            n.x_j = pos;
        }
    }

    /// Drops a neighbour along with everything [`DwMDS::observe`] accumulated
    /// about it, e.g. after it went silent for too long or after it was
    /// physically relocated and its ranging history no longer applies.
    pub fn forget(&mut self, id: u32) {
        self.x.remove(&id);
    }

    /// Number of neighbours contributing to the local cost. Every stored link
    /// has `w_ij > 0`, as [`DwMDS::observe`] never records a weightless one.
    pub fn neighbour_count(&self) -> usize {
        self.x.len()
    }

    /// Seeds the estimate with the weighted average of the neighbouring positions.
    /// Useful when a node has no prior at all.
    pub fn seed_from_neighbours(&mut self) {
        let mut sum = na::Vector3::zeros();
        let mut total = 0.;

        for n in self.x.values() {
            sum += n.effective_weight() * n.x_j;
            total += n.effective_weight();
        }

        if total > f32::EPSILON {
            self.x_i = sum / total;
        }
    }

    /// Local cost `S_i` (paper eq. 6), the part of the global stress that
    /// depends on `x_i`. Non-increasing over [`DwMDS::step`].
    ///
    /// Writing `ew_ij` for the effective weight of a link (`w_ij` for a peer,
    /// `2 w_ij` for an anchor)
    ///
    /// ```text
    /// S_i = sum_j ew_ij (delta_ij - d_ij)^2
    ///     + r_i ||x_i - xbar_i||^2
    /// ```
    pub fn cost(&self) -> f32 {
        let mut cost = self.r_i * (self.x_i - self.x_bar).norm_squared();

        for n in self.x.values() {
            let d_ij = (self.x_i - n.x_j).norm();
            let residual = n.delta_ij - d_ij;
            cost += n.effective_weight() * residual * residual;
        }

        cost
    }

    /// One SMACOF majorization step. Minimizes the quadratic majorant of the
    /// local cost at the current estimate, which never increases the cost.
    ///
    /// Writing `ew_ij` for the effective weight of a link (`w_ij` for a peer,
    /// `2 w_ij` for an anchor), p_ij for the pulling term and c_ij for
    /// each neighbours effect on the accumulated result,
    /// paper eq. (13) to (15) expand to
    ///
    /// ```text
    /// a_i = r_i + sum_j ew_ij
    /// p_ij = delta_ij / d_ij
    /// c_ij = ew_ij (x_j + p_ij (x_i - x_j))
    /// x_i <- (r_i xbar_i + sum_j c_ij) / a_i
    /// ```
    ///
    /// Returns the updated estimate.
    pub fn step(&mut self) -> na::Vector3<f32> {
        let mut a_i = self.r_i;
        let mut acc = self.r_i * self.x_bar;

        for n in self.x.values() {
            debug_assert!(n.w_ij > 0., "weightless link stored for a neighbour");

            let ew_ij = n.effective_weight();
            a_i += ew_ij;

            let offset = self.x_i - n.x_j;
            let d_ij = offset.norm();
            // The pull along the link is zero for coincident estimates, where
            // the direction to pull towards is undefined.
            let p_ij = if d_ij > f32::EPSILON {
                n.delta_ij / d_ij
            } else {
                0.
            };

            acc += ew_ij * (n.x_j + p_ij * offset);
        }

        // If enough valuable neighbours are present, update x_i from acc
        if a_i > f32::EPSILON {
            self.x_i = acc / a_i;
        }

        self.x_i
    }

    /// Iterates [`DwMDS::step`] until the relative cost decrease drops below
    /// [`Config::tolerance`] or [`Config::max_iterations`] is reached. This is
    /// the work a node does between two communication rounds, while its view
    /// of the neighbourhood is fixed.
    ///
    /// Returns the number of iterations performed.
    pub fn solve(&mut self) -> u32 {
        let mut previous = self.cost();

        for iteration in 0..self.config.max_iterations {
            self.step();
            let cost = self.cost();

            if previous - cost <= self.config.tolerance * previous {
                return iteration + 1;
            }
            previous = cost;
        }

        self.config.max_iterations
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four anchors spanning 3d space plus the exact ranges from `target`.
    fn anchored(target: na::Vector3<f32>, start: na::Vector3<f32>) -> DwMDS {
        let anchors = [
            na::Vector3::new(0., 0., 0.),
            na::Vector3::new(10., 0., 0.),
            na::Vector3::new(0., 10., 0.),
            na::Vector3::new(0., 0., 10.),
        ];

        let mut node = DwMDS::new(start, Default::default());
        for (id, anchor) in anchors.iter().enumerate() {
            node.observe_anchor(id as u32, *anchor, (target - anchor).norm(), 1.);
        }
        node
    }

    /// Deterministic pseudo-random jitter in `[-1, 1]`, so the noisy tests are
    /// reproducible without pulling in an RNG.
    fn jitter(seed: u32) -> f32 {
        let h = seed.wrapping_mul(2654435761) ^ (seed >> 15);
        (h % 2001) as f32 / 1000. - 1.
    }

    #[test]
    fn converges_to_the_exact_position() {
        let target = na::Vector3::new(3., 4., 2.);
        let mut node = anchored(target, na::Vector3::new(0., 0., 0.));

        node.solve();

        assert!((node.position() - target).norm() < 1e-3);
        assert!(node.cost() < 1e-4);
    }

    #[test]
    fn cost_never_increases() {
        let target = na::Vector3::new(3., 4., 2.);
        let mut node = anchored(target, na::Vector3::new(9., 1., 8.));

        let mut previous = node.cost();
        for _ in 0..100 {
            node.step();
            let cost = node.cost();
            assert!(cost <= previous + f32::EPSILON, "{cost} > {previous}");
            previous = cost;
        }
    }

    #[test]
    fn tolerates_noisy_ranges() {
        let target = na::Vector3::new(3., 4., 2.);
        let anchors = [
            (na::Vector3::new(0., 0., 0.), 0.2),
            (na::Vector3::new(10., 0., 0.), -0.15),
            (na::Vector3::new(0., 10., 0.), 0.1),
            (na::Vector3::new(0., 0., 10.), -0.2),
        ];

        let mut node = DwMDS::new(na::Vector3::new(5., 5., 5.), Default::default());
        for (id, (anchor, error)) in anchors.iter().enumerate() {
            node.observe_anchor(id as u32, *anchor, (target - anchor).norm() + error, 1.);
        }
        node.solve();

        assert!((node.position() - target).norm() < 0.5);
    }

    /// An anchor link enters `S_i` at `2 w_ij` and a peer link at `w_ij`
    /// (paper eq. 6), so an anchor outvotes an equally weighted peer 2:1.
    #[test]
    fn anchors_outweigh_equally_weighted_peers() {
        let anchor = na::Vector3::new(0., 0., 0.);
        let peer = na::Vector3::new(10., 0., 0.);

        // Both claim a range of 4 across a 10 m baseline, which cannot both
        // be true. Along the x axis the local cost is
        //   2 (4 - t)^2 + (t - 6)^2,
        // minimized at t = 14/3, rather than the t = 5 a symmetric treatment
        // of the two links would land on.
        let mut node = DwMDS::new(na::Vector3::new(5., 0., 0.), Default::default());
        node.observe_anchor(0, anchor, 4., 1.);
        node.observe(1, peer, 4., 1.);
        node.solve();

        assert!(
            (node.position().x - 14. / 3.).abs() < 1e-2,
            "{}",
            node.position()
        );
    }

    #[test]
    fn zero_weight_links_are_ignored() {
        let mut node = DwMDS::new(na::Vector3::zeros(), Default::default());
        node.observe(0, na::Vector3::new(100., 0., 0.), 100., 0.);

        assert_eq!(node.neighbour_count(), 0);

        // Nothing constrains the estimate, so it must stay put.
        node.solve();
        assert_eq!(node.position(), na::Vector3::zeros());
    }

    /// a link the caller trusts pulls harder than one it does not.
    #[test]
    fn weights_arbitrate_between_contradicting_ranges() {
        let trusted = na::Vector3::new(0., 0., 0.);
        let doubted = na::Vector3::new(10., 0., 0.);

        // Both claim a range of 4, which is impossible for a 10 m baseline:
        // the estimate has to land somewhere between 4 and 6 on the x axis.
        let mut node = DwMDS::new(na::Vector3::new(5., 0., 0.), Default::default());
        node.observe(0, trusted, 4., 10.);
        node.observe(1, doubted, 4., 0.1);
        node.solve();

        // Nearly all the residual is pushed onto the doubted link.
        assert!((node.position().x - 4.).abs() < 0.1, "{}", node.position());
    }

    /// The `K` exchanges with one neighbour collapse into a single weight and
    /// a single weight-averaged range, rather than the last one winning.
    #[test]
    fn repeated_ranging_averages_into_one_link() {
        let anchor = na::Vector3::zeros();

        // 3 m and 5 m, equally trusted, fuse to 4 m.
        let mut node = DwMDS::new(na::Vector3::new(1., 0., 0.), Default::default());
        node.observe_anchor(0, anchor, 3., 1.);
        node.observe_anchor(0, anchor, 5., 1.);
        assert_eq!(node.neighbour_count(), 1);

        node.solve();
        assert!((node.position().x - 4.).abs() <= f32::EPSILON, "{}", node.position());
    }

    /// Accumulated weight, not just an averaged range: two exchanges pull
    /// twice as hard as one.
    #[test]
    fn repeated_ranging_accumulates_weight() {
        let twice = na::Vector3::new(0., 0., 0.);
        let once = na::Vector3::new(10., 0., 0.);

        // The same contradiction as `anchors_outweigh_equally_weighted_peers`,
        // but the 2:1 now comes from two exchanges against one instead of from
        // the anchor rule: cost 2 (4 - t)^2 + (t - 6)^2, minimized at t = 14/3.
        let mut node = DwMDS::new(na::Vector3::new(5., 0., 0.), Default::default());
        node.observe(0, twice, 4., 1.);
        node.observe(0, twice, 4., 1.);
        node.observe(1, once, 4., 1.);
        node.solve();

        assert!(
            (node.position().x - 14. / 3.).abs() < 1e-2,
            "{}",
            node.position()
        );
    }

    /// A relocated neighbour has to be dropped, otherwise its ranging history
    /// keeps voting for where it used to be.
    #[test]
    fn forget_resets_the_accumulator() {
        let anchor = na::Vector3::zeros();

        let mut node = DwMDS::new(na::Vector3::new(1., 0., 0.), Default::default());
        for _ in 0..10 {
            node.observe_anchor(0, anchor, 3., 1.);
        }
        node.forget(0);
        node.observe_anchor(0, anchor, 5., 1.);

        node.solve();
        assert!((node.position().x - 5.).abs() <= f32::EPSILON, "{}", node.position());
    }

    #[test]
    fn prior_holds_an_unconstrained_node_in_place() {
        let prior = na::Vector3::new(1., 2., 3.);
        let mut node = DwMDS::with_prior(
            prior,
            1.,
            Default::default()
        );

        // A single range leaves the position on a sphere; the prior picks the
        // point on it that stays closest to where we started. Along the x axis
        // the cost is (t - 1)^2 + c (2 - t)^2 with c = 2 w_ij = 2 for the
        // anchor, minimized at t = (1 + 2c) / (1 + c) = 5/3. The exact value
        // pins how far the prior is allowed to be outvoted.
        node.observe_anchor(0, na::Vector3::new(0., 2., 3.), 2., 1.);
        node.solve();

        let expected = na::Vector3::new(5. / 3., 2., 3.);
        assert!((node.position() - expected).norm() <= f32::EPSILON, "{}", node.position());
    }

    /// Two unknown nodes localizing each other against a shared anchor set,
    /// exchanging estimates after every local solve.
    #[test]
    fn neighbours_converge_over_communication_rounds() {
        let truth = [
            na::Vector3::new(2., 3., 1.),
            na::Vector3::new(6., 2., 4.),
        ];
        let anchors = [
            (10, na::Vector3::new(0., 0., 0.)),
            (11, na::Vector3::new(10., 0., 0.)),
            (12, na::Vector3::new(0., 10., 0.)),
        ];

        let mut nodes = [
            DwMDS::new(na::Vector3::new(0., 0., 5.), Default::default()),
            DwMDS::new(na::Vector3::new(9., 9., 0.), Default::default()),
        ];

        // One ranging round up front. Re-observing every iteration would
        // accumulate the same measurement over and over and skew the
        // anchor/peer balance.
        for (i, node) in nodes.iter_mut().enumerate() {
            for (id, anchor) in anchors {
                node.observe_anchor(id, anchor, (truth[i] - anchor).norm(), 1.);
            }
            let peer = 1 - i;
            let start = na::Vector3::zeros();
            node.observe(peer as u32, start, (truth[i] - truth[peer]).norm(), 1.);
        }

        for _ in 0..20 {
            // Broadcast: each node publishes what it currently believes.
            let broadcast = [nodes[0].position(), nodes[1].position()];

            for (i, node) in nodes.iter_mut().enumerate() {
                node.set_neighbour_position((1 - i) as u32, broadcast[1 - i]);
                node.solve();
            }
        }

        let tolerance = Config::default().tolerance;
        for (i, (node, expected)) in nodes.iter().zip(truth).enumerate() {
            assert!((node.position() - expected).norm() <= tolerance, "node {}: {}", i, node.position());
        }
    }

    /// Two unknown nodes localizing each other against a shared anchor set,
    /// even if ranges are noisy.
    #[test]
    fn neighbours_converge_over_noisy_communication_rounds() {
        let noise = 1.;
        let truth = [
            na::Vector3::new(2., 3., 1.),
            na::Vector3::new(6., 2., 4.),
        ];
        let anchors = [
            (10, na::Vector3::new(0., 0., 0.)),
            (11, na::Vector3::new(10., 0., 0.)),
            (12, na::Vector3::new(0., 10., 0.)),
        ];

        let mut nodes = [
            DwMDS::new(na::Vector3::new(0., 0., 5.), Default::default()),
            DwMDS::new(na::Vector3::new(9., 9., 0.), Default::default()),
        ];

        for round in 0..20 {
            // Broadcast: each node publishes what it currently believes.
            let broadcast = [nodes[0].position(), nodes[1].position()];

            for (i, node) in nodes.iter_mut().enumerate() {
                for (id, anchor) in anchors {
                    let range = (truth[i] - anchor).norm() + noise * jitter(i as u32 * 97 + id * 45 + round);
                    node.observe_anchor(id, anchor, range, 1.);
                }

                let peer = 1 - i;
                let range = (truth[i] - truth[peer]).norm() + noise * jitter(i as u32 * 97 + round);

                node.observe(peer as u32, broadcast[peer], range, 1.);

                node.solve();
            }
        }

        for (i, (node, expected)) in nodes.iter().zip(truth).enumerate() {
            assert!((node.position() - expected).norm() <= noise, "node {}: {}", i, node.position());
        }
    }
}
