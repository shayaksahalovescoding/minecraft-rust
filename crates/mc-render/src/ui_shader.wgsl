struct UiUniforms {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> ui_u: UiUniforms;

@group(1) @binding(0)
var ui_tex: texture_2d<f32>;
@group(1) @binding(1)
var ui_samp: sampler;

struct UiVsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct UiVsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_ui(in: UiVsIn) -> UiVsOut {
    var out: UiVsOut;
    let ndc_x = (in.pos.x / ui_u.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (in.pos.y / ui_u.screen_size.y) * 2.0;
    out.clip_pos = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fs_ui(in: UiVsOut) -> @location(0) vec4<f32> {
    let t = textureSample(ui_tex, ui_samp, in.uv);
    let col = t * in.color;
    if (col.a <= 0.005) {
        discard;
    }
    return col;
}
