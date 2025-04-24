use cgmath::*;
use ecs::*;
use ecs_derive::*;
use map::vecmap::VecMap;

#[derive(Copy, Clone, Component)]
pub struct Transform(Matrix4<f32>);
#[derive(Copy, Clone, Component)]
pub struct Position(Vector3<f32>);

#[derive(Copy, Clone, Component)]
pub struct Rotation(Vector3<f32>);

#[derive(Copy, Clone, Component)]
pub struct Velocity(Vector3<f32>);