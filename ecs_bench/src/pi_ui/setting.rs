use std::{fs::DirEntry, path::Path, mem::transmute};
use std::sync::Arc;

use pi_async::prelude::{WorkerRuntime, AsyncRuntime, AsyncRuntimeBuilder};

use pi_ecs::prelude::{Setup, IntoSystem, System, World, DispatcherMgr, StageBuilder, SingleDispatcher};
use pi_hash::XHashMap;
use pi_idtree::IdTree;
use pi_style::style_type::ClassSheet;
use pi_ui_render::gui::Gui;
use pi_ui_render::system::node::flush::CalcFlush;
use pi_ui_render::{export::{*, json_parse::as_value}, system::node::{user_setting::CalcUserSetting, flush::CmdCache}, utils::cmd::SingleCmd};
use json::{number::Number, object::Object, JsonValue};
use pi_map::vecmap::VecMap;
use share::Share;
use winit::event_loop::EventLoopBuilder;
use winit::platform::windows::EventLoopBuilderExtWindows;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
};

pub struct Benchmark{
	engine: Engine, 
	system: Box<dyn System<In=(), Out = ()>>,
}


impl Benchmark {
    pub fn new() -> Self {
		let size = (1000, 1000);

		let mut engine = create_engine();

		let mut play_context = PlayContext {
			nodes: VecMap::new(),
			idtree: IdTree::default(),
			atoms: XHashMap::default(),
		};
		let mut list_index = 0;
		let mut file_index = 0;
		let play_version = "1669263770735";
		let play_path = "G://pi_demo_m/dst";
		let cmd_path = Some("G://pi_demo_m/dst");
		let mut json_arr = JsonValue::Array(Vec::default());
		// width: 400,
		// height: 750,
		// scale: 1.0
		let width = 1024;
		let height = 1920;
		let scale = 0.5;
		let size = (1000, 800);

		let mut flush = IntoSystem::system(CalcFlush::user_setting, &mut engine.gui.world);
		let mut system = Box::new( IntoSystem::system(CalcUserSetting::user_setting, &mut engine.gui.world));

		std::env::set_current_dir(play_path).unwrap();
		let dir = std::env::current_dir().unwrap();
		log::warn!("current_dir: {:?}", dir);

        // 设置class
		let mut class_sheet = ClassSheet::default();
        let mut cb = |dwcss: &DirEntry| {
			if let Some(r)  = dwcss.path().extension() {
				if r != "dcss" {
					return;
				}
			} else {
				return;
			}
            let file = std::fs::read(dwcss.path());
            if let Ok(r) = file {
				create_class_by_bin(&mut engine, r.as_slice());
                // let file = String::from_utf8(r).unwrap();
                // let mut r = parse_class_map_from_string(file.as_str(), 0).unwrap();
                // gui.gui.push_cmd(SingleCmd(std::mem::replace(&mut r.key_frames, KeyFrameList::default())));
                // r.to_class_sheet(&mut class_sheet);
            }
        };
        visit_dirs(&Path::new(play_path), &mut cb).unwrap();

        let full_screen_class = format!(
            ".c3165071837 {{position : absolute ;left : 0px ;top : 0px ;width : {:?}px ;height : {:?}px ;}}",
            width, height
        );
        let full_screen_class = pi_style::style_parse::parse_class_map_from_string(full_screen_class.as_str(), 0).unwrap().to_class_sheet(&mut class_sheet);
		engine.gui.push_cmd(SingleCmd(class_sheet));

		flush.run(());
		system.run(());


        // let gui = &mut gui.gui;
        // let gui = unsafe { &mut *(gui as *mut Gui as usize as *mut pi_ui_render::export::Gui)};
        let context = &mut play_context;
        context.atoms.insert(3781626326, Atom::new(pi_atom::Atom::from("_$text")));
        context.atoms.insert(11, Atom::new(pi_atom::Atom::from("")));

        let mut json = Object::new();
		let id: f64 = unsafe{transmute::<u64, f64>(1 + (1 << 32))};
        json.insert("ret", JsonValue::Number(id.into()));
        let root = play_create_node(&mut engine, context, &vec![JsonValue::Object(json.clone())]);
        play_width(
            &mut engine,
            context,
            &vec![JsonValue::Number(Number::from(id)), JsonValue::Number(Number::from(width))],
        );
        play_height(
            &mut engine,
            context,
            &vec![JsonValue::Number(Number::from(id)), JsonValue::Number(Number::from(height))],
        );
        play_transform_scale(
            &mut engine,
            context,
            &vec![
                JsonValue::Number(Number::from(id)),
                JsonValue::Number(Number::from(scale)),
                JsonValue::Number(Number::from(scale)),
            ],
        );
        play_transform_origin(
            &mut engine,
            context,
            &vec![
                JsonValue::Number(Number::from(id)),
                JsonValue::Number(Number::from(0)),
                JsonValue::Number(Number::from(0.0)),
                JsonValue::Number(Number::from(0)),
                JsonValue::Number(Number::from(0.0)),
            ],
        );
        play_position(
            &mut engine,
            context,
            &vec![
                JsonValue::Number(Number::from(id)),
                JsonValue::Number(Number::from(0)),
                JsonValue::Number(Number::from(0.)),
            ],
        );
        play_position(
            &mut engine,
            context,
            &vec![
                JsonValue::Number(Number::from(id)),
                JsonValue::Number(Number::from(1)),
                JsonValue::Number(Number::from(0.)),
            ],
        );
        play_margin(
            &mut engine,
            context,
            &vec![
                JsonValue::Number(Number::from(id)),
                JsonValue::Number(Number::from(0)),
                JsonValue::Number(Number::from(0.)),
            ],
        );
        play_margin(
            &mut engine,
            context,
            &vec![
                JsonValue::Number(Number::from(id)),
                JsonValue::Number(Number::from(1)),
                JsonValue::Number(Number::from(0.)),
            ],
        );
        play_position_type(
            &mut engine,
            context,
            &vec![JsonValue::Number(Number::from(id)), JsonValue::Number(Number::from(1))],
        );
        play_append_child(
            &mut engine,
            context,
            &vec![JsonValue::Number(Number::from(id)), JsonValue::Number(Number::from(0))],
        );

		while setting(&mut list_index, &mut json_arr, cmd_path, play_path, play_version, &mut file_index, &mut engine, &mut play_context) {}

		flush.run(());

		// 设置

		// user_setting
        Self{
			engine,
			system,	
		}
    }

    pub fn run(mut s: Self) {
		s.system.run(())
    }
}

pub fn visit_dirs<F: FnMut(&DirEntry)>(path: &Path, cb: &mut F) -> std::io::Result<()> {
    if path.is_dir() {
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}

pub fn setting(list_index1: &mut usize, json_arr: &mut JsonValue, cmd_path: Option<&str>, play_path: &str, play_version: &str, file_index1: &mut usize, app: &mut Engine, play_context: &mut PlayContext) -> bool{
	let (mut list_index, mut file_index) = (*list_index1, *file_index1);
	if list_index >= json_arr.len() {
		if list_index == json_arr.len() {
			let dir = match cmd_path {
				Some(r) => r.to_string(),
				None => play_path.to_string(),
			};
			let path = dir + "/gui_cmd/cmd_" + play_version + "_" + file_index.to_string().as_str() + ".gui_cmd.json";
			match std::fs::read(path.clone()) {
				Ok(r) => {
					*json_arr = json::parse(String::from_utf8(r).unwrap().as_str()).unwrap();
					list_index = 0;
					file_index += 1;
					*list_index1 = list_index;
					*file_index1 = file_index;
				}
				Err(_) =>  {
					log::warn!("play end, {:?}", path);
					return false;
				},
			};
		}
	}

	if list_index < json_arr.len() {
		let cur_play = &json_arr[list_index];
		if let JsonValue::Array(cur_play) = cur_play {
			for play_item in cur_play.iter() {
				if let JsonValue::Object(r) = play_item {
					let ty = r.get("type").unwrap().as_usize().unwrap();
					let param = r.get("param").unwrap();
					let ret = match r.get("ret") {
						Some(r) => match r.as_f64() {
							Some(r) => r,
							None => 0.0,
						},
						None => 0.0,
					};

					if ret == 0.0 {
						if let JsonValue::Array(param) = param {
							if let Some(cmd) = CMD_LIST.get(ty) {
								cmd(app, play_context, param);
							}
						}
					} else {
						if let Some(cmd) = CMD_LIST.get(ty) {
							cmd(app, play_context, &vec![play_item.clone()]);
						}
					}
				}
			}
		}
	}
	*list_index1 += 1;
	
	return  true;
}



lazy_static::lazy_static! {
    pub static ref CMD_LIST: Vec<fn (&mut Engine, &mut PlayContext, &Vec<json::JsonValue>) > = vec![
        // 布局
        play_position_type, // 1
        play_display, // 2

        play_width, // 3
        play_width_auto, // 4
        play_width_percent, // 5
        play_min_width, // 6
        play_min_width_percent, // 7
        play_max_width, // 8
        play_max_width_percent, // 9

        play_height, // 10
        play_height_auto, // 11
        play_height_percent, // 12
        play_min_height, // 13
        play_min_height_percent, // 14
        play_max_height, // 15
        play_max_height_percent, // 16

        play_position, // 17
        play_position_percent, // 18

        play_margin, // 19
        play_margin_auto, // 20
        play_margin_percent, // 21

        play_padding, // 22
        play_padding_percent, // 23

        play_border, // 24

        play_flex_direction, // 25
        play_align_content, // 26
        play_align_items, // 27
        play_align_self, // 28
        play_justify_content, // 29

        play_flex_wrap, // 30
        play_flex_grow, // 31
        play_flex_shrink, // 32
        play_flex_basis, // 33
        play_flex_basis_auto, // 34
        play_todo, //play_align_content, // 不存在，暂时占位35

        // offset，会导致布局，也录制下来
        play_todo, //"offset_top",36
        play_todo, //"offset_left",37
        play_todo, //"offset_width",38
        play_todo, //"offset_height",39
        play_todo, //"offset_ducument",40

        // transform
        play_clear_transform, //"clear_transform",41
        play_clear_transform, //"reset_transform",42

        play_transform_translate, // 43
        play_transform_translate_x, // 44
        play_transform_translate_y, // 45

        play_transform_translate_percent, // 46
        play_transform_translate_x_percent, // 47
        play_transform_translate_y_percent, // 48

        play_transform_scale, // 49
        play_transform_scale_x, // 50
        play_transform_scale_y, // 51

        play_transform_rotate_x, // 52
        play_transform_rotate_y, // 53
        play_transform_rotate_z, // 54

        play_transform_skew_x, // 55
        play_transform_skew_y, // 56

        play_transform_origin, // 57

        // "create_engine",
        // "create_gui",

        // play_view_port,
        // play_project_transfrom,
        // play_gui_size,

        play_create_node, // 57
        play_create_vnode, // 58
        play_create_text_node, // 59
        play_create_image_node, // 60
        play_create_canvas_node, // 61

        play_remove_node, // 62
        play_destroy_node, // 63

        play_todo, //"update_canvas",64
        play_todo, //play_canvas_size,65

        play_todo, //"force_update_text",66
        play_todo, //play_render_dirty,67
        play_todo, //"render",68
        play_todo, //"calc",69
        play_todo, //"calc_geo",70
        play_todo, //"cal_layout",71
        // "create_render_target",
        // "bind_render_target",

        play_todo, //"add_canvas_font",72
        play_todo, //"add_sdf_font_res",73
        play_todo, //"add_font_face",74

        play_todo, //play_transform_will_change,75

        play_set_class, //play_class,76
        play_todo, //"add_class_start",
        play_todo, //"add_class",
        play_todo, //"add_class_end",
        play_set_class, //play_class_name,
        play_todo, //play_default_style_by_bin,

        play_filter_hsi, // 1
        play_enable, // 1

        play_append_child, // 84
        play_insert_before, // 85
        play_todo, // "insert_after", // 86

        play_todo, // "first_child",
        play_todo, // "last_child",
        play_todo, // "next_sibling",
        play_todo, // "previous_sibling",
        play_todo, // "node_is_exist",

        play_background_rgba_color,
        play_todo, //play_background_radial_gradient_color,
        play_background_linear_color,

        play_background_image, //play_src,
        play_image_clip, // 1
        play_object_fit, // 1

        play_mask_image, // 1
        play_mask_image_clip, // 1
        play_mask_image_linenear, // 1

        play_border_color, // 1
        play_border_radius, // 1
        play_border_image, // 1
        play_border_image_slice, // 1
        play_border_image_clip, // 1
        play_border_image_repeat, // 1

        play_blend_mode, // 1

        play_overflow, // 1
        play_opacity, // 1
        play_zindex, // 1
        play_visibility, // 1

        play_todo, //"text",
        play_text_content,
        play_todo, //play_clip_path_geometry_box,
        play_todo, //play_clip_path_basic_shape,
        play_todo, //"text_align",
        play_text_align,
        play_todo, //"letter_spacing",
        play_letter_spacing,
        play_todo, //"line_height",
        play_line_height,
        play_todo, //"text_indent",
        play_text_indent,
        play_todo, //"white_space",
        play_white_space, // 1
        play_text_stroke, // 1
        play_text_linear_gradient_color, // 1
        play_text_shadow, // 1
        play_text_rgba_color, // 1
        play_todo, //"font",
        play_todo, //"font_style",
        play_font_style,
        play_todo, //"font_weight",
        play_font_weight,
        play_todo, //"font_size",
        play_font_size,
        play_todo, //"font_family",
        play_font_family, // 1

        play_box_shadow, // 1
        play_todo, //play_box_shadow_color,
        play_todo, //play_box_shadow_h,
        play_todo, //play_box_shadow_v,
        play_todo, //play_box_shadow_blur,

        play_reset_text_content, //"reset_text_content",
        play_reset_font_style, //"reset_font_style",
        play_reset_font_weight, //"reset_font_weight",
        play_reset_font_size, //"reset_font_size",
        play_reset_font_family, //"reset_font_family",
        play_reset_letter_spacing, //"reset_letter_spacing",
        play_reset_word_spacing, //"reset_word_spacing",
        play_reset_line_height, //"reset_line_height",
        play_reset_text_indent, //"reset_indent",
        play_reset_white_space, //"reset_white_space",
        play_reset_text_align, //"reset_text_align",
        play_todo, //"reset_vertical_align",
        play_reset_text_rgba_color, //"reset_color",
        play_reset_text_stroke, //"reset_stroke",
        play_reset_text_shadow, //"reset_text_shadow",
        play_reset_background_image, //"reset_image",
        play_reset_image_clip, //"reset_image_clip",
        play_reset_object_fit, //"reset_object_fit",
        play_reset_border_image, //"reset_border_image",
        play_reset_border_image_clip, //"reset_border_image_clip",
        play_reset_border_image_slice, //"reset_border_image_slice",
        play_reset_border_image_repeat, //"reset_border_image_repeat",
        play_reset_border_color, //"reset_border_color",
        play_reset_border_radius, //"reset_border_radius",
        play_reset_background_rgba_color, //"reset_background_color",
        play_reset_box_shadow, //"reset_box_shadow",
        play_todo, //"reset_filter",
        play_reset_opacity, //"reset_opacity",
        play_reset_flex_direction, //"reset_direction",
        play_todo, //"reset_order",
        play_reset_flex_basis, //"reset_flex_basis",
        play_reset_zindex, //"reset_z_index",
        play_todo, //"reset_transform",
        play_todo, //"reset_transform_will_change",
        play_reset_overflow, //"reset_overflow",
        play_reset_mask_image, //"reset_mask_image",
        play_reset_mask_image_clip, //"reset_mask_image_clip",
        play_reset_width, //"reset_width",
        play_reset_height, //"reset_height",
        play_todo, //"reset_margin_top",
        play_todo, //"reset_margin_right",
        play_todo, //"reset_margin_bottom",
        play_todo, //"reset_margin_left",
        play_todo, //"reset_top",
        play_todo, //"reset_right",
        play_todo, //"reset_bottom",
        play_todo, //"reset_left",
        play_todo, //"reset_padding_top",
        play_todo, //"reset_padding_right",
        play_todo, //"reset_padding_bottom",
        play_todo, //"reset_padding_left",
        play_todo, //"reset_border_top",
        play_todo, //"reset_border_right",
        play_todo, //"reset_border_bottom",
        play_todo, //"reset_border_left",
        play_reset_min_width, //"reset_min_width",
        play_reset_min_height, //"reset_min_height",
        play_reset_max_width, //"reset_max_width",
        play_reset_max_height, //"reset_max_height",
        play_reset_justify_content, //"reset_justify_content",
        play_reset_flex_shrink, //"reset_flex_shrink",
        play_reset_flex_grow, //"reset_flex_grow",
        play_reset_position_type, //"reset_position_type",
        play_reset_flex_wrap, //"reset_flex_wrap",
        play_reset_flex_direction, //"reset_flex_direction",
        play_reset_align_content, //"reset_align_content",
        play_reset_align_items, //"reset_align_items",
        play_reset_align_self, //"reset_align_self",
        play_reset_blend_mode, //"reset_blend_mode",
        play_reset_display, //"reset_display",
        play_reset_visibility, //"reset_visibility",
        play_reset_enable, //"reset_enable",


        set_atom, //"__$set_atom",
    ];

	// pub static ref CREATE_CMD_LIST: Vec<fn (&mut App, &mut PlayContext, &Vec<json::JsonValue>) > = vec![
    //     // 布局
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, //play_align_content, // 不存在，暂时占位

    //     // offset，会导致布局，也录制下来
    //     play_todo_create, //"offset_top",
    //     play_todo_create, //"offset_left",
    //     play_todo_create, //"offset_width",
    //     play_todo_create, //"offset_height",
    //     play_todo_create, //"offset_ducument",

    //     // transform
    //     play_todo_create, //"clear_transform",
    //     play_todo_create, //"reset_transform",

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1
    //     play_todo_create, // 1

    //     play_todo_create, // 1

    //     // "create_engine",
    //     // "create_gui",

    //     // play_view_port,
    //     // play_project_transfrom,
    //     // play_gui_size,

    //     play_create_node, // 57
    //     play_create_vnode, // 58
    //     play_create_text_node, // 59
    //     play_create_image_node, // 60
    //     play_create_canvas_node, // 61
    // ];
}

pub fn play_todo(_gui: &mut Engine, _context: &mut PlayContext, _json: &Vec<json::JsonValue>) {}
// pub fn play_todo_create(_gui: &mut Engine, _context: &mut PlayContext, _json: &Vec<json::JsonValue>) {}

// pub fn render(gui: &mut pi_ui_render::export::Gui, context: &mut PlayContext, _json: &Vec<json::JsonValue>) {
// 	{
// 		gui.0.run();
// 		context.dispatcher.0.borrow_mut().run();
// 	}

// 	// 睡眠16毫秒
// 	std::thread::sleep( Duration::from_millis(16));
// }

pub fn set_atom(_gui: &mut Engine, context: &mut PlayContext, json: &Vec<json::JsonValue>) {
    // 这里必须要在json中存在两个字段，分别是hash和字符串，而不能只有字符串
    // 因为hash有其他地方生成，比如32位的wasm生成，与当前64位程序计算出来的hash不同
    let key = as_value::<usize>(json, 0).unwrap();

    let v = as_value::<String>(json, 1).unwrap();
    let value = Atom::new(pi_atom::Atom::from(v));
    context.atoms.insert(key, value);
}

pub fn create_engine() -> Engine {

    let runtime = AsyncRuntimeBuilder::default_worker_thread(None, None, None, None);

    let mut world = World::new();

    let rt = runtime.clone();

	let gui = Gui::new(&mut world);
	let mut dispatcher_mgr = DispatcherMgr::default();
	
	let mut node_stage = StageBuilder::new();
	CalcUserSetting::setup(&mut world, &mut node_stage);
	

	let mut calc_dispacher = SingleDispatcher::new(runtime.clone());
	calc_dispacher.init(vec![Share::new(node_stage.build(&mut world))], &mut world);
	dispatcher_mgr.insert(Box::new(calc_dispacher));
	
	Engine {
		render_dispatcher: Default::default(),
		dispatcher_mgr,
		world: world,
		rt,
		gui,
	}
}



#[test]
fn test() {
	env_logger::Builder::default()
        // .filter(Some("wgpu_core"), log::LevelFilter::Warn)
        // .filter(Some("wgpu_hal"), log::LevelFilter::Warn)
        // .filter(Some("pi_graph"), log::LevelFilter::Warn)
        .filter(None, log::LevelFilter::Warn)
        // .filter(Some("pi_ui_render"), log::LevelFilter::Trace)
        // .filter(Some("pi_animation"), log::LevelFilter::Trace)
        // .filter(Some("pi_curves"), log::LevelFilter::Trace)
		// .filter(Some("pi_flex_layout"), log::LevelFilter::Trace)
		// .filter(Some("pi_style::style_type"), log::LevelFilter::Trace)
		// .filter(Some("pi_ui_render::components::user"), log::LevelFilter::Trace)
		.filter(Some("pi_hal"), log::LevelFilter::Trace)
		.filter(Some("pi_render"), log::LevelFilter::Trace)
        .filter(Some("pi_ui_render"), log::LevelFilter::Info)
		.filter(Some("pi_ui_ecs"), log::LevelFilter::Info)
        .init();
		
	let r = Benchmark::new();
	Benchmark::run(r);
}
