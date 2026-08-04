mod dw_mds;
use dw_mds::DwMDS;

use uwb::hal::UwbHal;

use nalgebra as na;

const MAX_FRAME_SIZE: usize = 64;

pub struct Node<TRC>
where TRC: UwbHal<MAX_FRAME_SIZE> {
    uwb_tranceiver: TRC,
    mds_solver: DwMDS,
}

impl<TRC> Node<TRC>
where TRC: UwbHal<MAX_FRAME_SIZE> {
    pub fn new(initial: na::Vector3<f32>, uwb_tranceiver: TRC) -> Self {
        let mds_solver = DwMDS::new(initial, Default::default());
        Self {
            uwb_tranceiver,
            mds_solver,
        }
    }

    pub fn update(&mut self) {

    }
}
