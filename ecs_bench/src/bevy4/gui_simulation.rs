use bevy_4_ecs::prelude::*;
use cgmath::*;

#[derive(Copy, Clone, Default)]
struct Transform(Matrix4<f32>);
#[derive(Copy, Clone, Default)]
pub struct RectLayoutStyle ([f32; 6]);
#[derive(Copy, Clone, Default)]
pub struct OtherLayoutStyle ([f32; 15]);
#[derive(Copy, Clone, Default)]
pub struct ZIndex(pub isize);
#[derive(Copy, Clone, Default)]
pub struct Overflow(pub bool);
#[derive(Copy, Clone, Default)]
pub struct Opacity(pub f32);
#[derive(Copy, Clone, Default)]
pub struct Show(pub usize);
#[derive(Copy, Clone, Default)]
pub struct BackgroundColor([f32; 4]);
#[derive(Copy, Clone, Default)]
pub struct ClassName {
    pub one: usize,
    pub two: usize,
    pub other: Vec<usize>,
}
#[derive(Copy, Clone, Default)]
pub struct BorderColor([f32; 4]);
#[derive(Copy, Clone, Default)]
pub struct Image {
    pub url: usize,
    // canvas使用
    pub width: Option<f32>,
    pub height: Option<f32>,
}
#[derive(Copy, Clone, Default)]
pub struct MaskImage {
	pub url: usize,
}
#[derive(Copy, Clone, Default)]
pub struct MaskImageClip ([f32; 6]);
#[derive(Copy, Clone, Default)]
pub struct Filter {
    pub hue_rotate: f32,  //色相转换  -0.5 ~ 0.5 , 对应ps的-180 ~180
    pub saturate: f32,    // 饱和度  -1。0 ~1.0 ， 对应ps的 -100 ~ 100
    pub bright_ness: f32, //亮度 -1。0 ~1.0 ， 对应ps的 -100 ~ 100
}
#[derive(Copy, Clone, Default)]
pub struct ObjectFit(pub usize);
#[derive(Copy, Clone, Default)]
pub struct ImageClip(pub [f32; 4]);
#[derive(Copy, Clone, Default)]
pub struct BorderImage(pub Image);
#[derive(Copy, Clone, Default)]
pub struct BorderImageClip(pub ImageClip);
#[derive(Copy, Clone, Default)]
pub struct BorderImageSlice {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
    pub fill: bool,
}
#[derive(Copy, Clone, Default)]
pub struct BorderImageRepeat([f32; 2]);
#[derive(Copy, Clone, Default)]
pub struct BorderRadius {
    pub x: usize,
    pub y: usize,
}
#[derive(Copy, Clone, Default)]
pub struct BoxShadow {
    pub h: f32,         // 水平偏移，正右负左
    pub v: f32,         // 垂直偏移，正下负上
    pub blur: f32,      // 模糊半径，0代表不模糊，
    pub spread: f32,    // 阴影扩展，上下左右各加上这个值
    pub color: [f32; 4], // 阴影颜色
}
#[derive(Copy, Clone, Default)]
pub struct Text {
    pub letter_spacing: f32,     //字符间距， 单位：像素
    pub word_spacing: f32,       //字符间距， 单位：像素
    pub line_height: u8, //设置行高
    pub indent: f32,             // 缩进， 单位： 像素
    pub white_space: u8, //空白处理
    pub color: [f32;4],            //颜色
    pub stroke: f32,
    pub text_align: u8,
    pub vertical_align: u8,
}
#[derive(Copy, Clone, Default)]
pub struct TextContent(pub String, pub usize);
#[derive(Copy, Clone, Default)]
pub struct TextStyle {
    pub text: Text,
    pub font: Font,
    pub shadow: TextShadow,
}
#[derive(Copy, Clone, Default)]
pub struct TransformWillChange(pub Transform);


#[derive(Copy, Clone, Default)]
pub struct TextShadow {
    pub h: f32,         //	必需。水平阴影的位置。允许负值。	测试
    pub v: f32,         //	必需。垂直阴影的位置。允许负值。	测试
    pub blur: f32,      //	可选。模糊的距离。	测试
    pub color: CgColor, //	可选。阴影的颜色。参阅 CSS 颜色值。
}
#[derive(Copy, Clone, Default)]
pub struct Font {
    pub style: FontStyle, //	规定字体样式。参阅：font-style 中可能的值。
    pub weight: usize,    //	规定字体粗细。参阅：font-weight 中可能的值。
    pub size: FontSize,   //
    pub family: Atom,     //	规定字体系列。参阅：font-family 中可能的值。
}


pub struct Benchmark(World);

impl Benchmark {
    pub fn new() -> Self {
        let mut world = World::new();
        world.spawn_batch((0..10_000).map(|_| {
            (
                Transform(Matrix4::from_scale(1.0)),
                Position(Vector3::unit_x()),
                Rotation(Vector3::unit_x()),
                Velocity(Vector3::unit_x()),
            )
        }));

        Self(world)
    }

    pub fn run(&mut self) {
        for (velocity, mut position) in self.0.query_mut::<(&Velocity, &mut Position)>() {
            position.0 += velocity.0;
        }
    }
}


