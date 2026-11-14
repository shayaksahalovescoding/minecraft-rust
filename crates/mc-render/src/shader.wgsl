struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    fog_color: vec4<f32>,  // rgb = fog color, a = sky brightness (0.08..1.0)
    fog_params: vec4<f32>, // x = fog_start, y = fog_end, z = alpha_cutoff, w = unlit_mode (0=lit, 1=unlit)
};

@group(0) @binding(0)
var<uniform> u: Uniforms;

@group(1) @binding(0)
var atlas_tex: texture_2d<f32>;
@group(1) @binding(1)
var atlas_samp: sampler;

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) light: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light: vec2<f32>,
    @location(3) dist: f32,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.clip_pos = u.view_proj * vec4<f32>(in.pos, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.light = in.light;
    out.dist = length(in.pos - u.camera_pos.xyz);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(atlas_tex, atlas_samp, in.uv);
    let alpha = texel.a * in.color.a;
    if (alpha < u.fog_params.z) {
        discard;
    }

    var rgb = texel.rgb * in.color.rgb;
    if (u.fog_params.w < 0.5) {
        let sky_l = clamp(in.light.x * u.fog_color.a, 0.0, 1.0);
        let blk_l = clamp(in.light.y, 0.0, 1.0);
        let sky_rgb = vec3<f32>(0.97, 0.99, 1.0) * (0.055 + 0.945 * pow(sky_l, 1.25));
        let blk_rgb = vec3<f32>(1.0, 0.90, 0.72) * pow(blk_l, 1.35);
        rgb = rgb * max(sky_rgb, blk_rgb);
    }

    if (u.fog_params.y > u.fog_params.x && u.fog_params.w < 0.5) {
        let f = clamp((in.dist - u.fog_params.x) / (u.fog_params.y - u.fog_params.x), 0.0, 1.0);
        rgb = mix(rgb, u.fog_color.rgb, f * f);
    }

    return vec4<f32>(rgb, alpha);
}
