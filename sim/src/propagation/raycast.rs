use godot::{
    classes::{PhysicsDirectSpaceState3D, PhysicsRayQueryParameters3D},
    prelude::*,
};

use crate::propagation::link::PathInfo;

pub fn calc_direct_path(
    mut space_state: Gd<PhysicsDirectSpaceState3D>,
    from: Vector3,
    to: Vector3,
    collision_mask: u32,
) -> Option<PathInfo> {
    let real_distance_m = from.distance_to(to);

    let mut query = PhysicsRayQueryParameters3D::create(from, to)?;
    query.set_collision_mask(collision_mask);
    query.set_collide_with_areas(false);
    query.set_collide_with_bodies(true);

    let hit = space_state.intersect_ray(&query);

    Some(PathInfo {
        real_distance_m,
        has_los: hit.is_empty(),
    })
}
