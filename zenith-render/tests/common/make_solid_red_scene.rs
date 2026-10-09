use zenith_scene::{Color, Paint, Scene, SceneCommand};

pub fn make_solid_red_scene(page: f64) -> Scene {
    let mut s = Scene::new(page, page);
    s.commands.push(SceneCommand::PushClip {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
    });
    s.commands.push(SceneCommand::FillRect {
        x: 0.0,
        y: 0.0,
        w: page,
        h: page,
        paint: Paint::solid(Color::srgb(255, 0, 0, 255)),
    });
    s.commands.push(SceneCommand::PopClip);
    s
}
