//!HOOK OUTPUT
//!BIND HOOKED
//!DESC SDR shadow contrast and chroma

// The native SDR surface is sRGB. Reconstruct the BT.1886-coded Rec.709
// values used by the export, apply the same limited-code luma/chroma
// adjustment as the export LUT, then return to sRGB for Windows composition.
// Apply the curve relative to black, preserving detail instead of clipping
// legal near-black video codes below their nominal range.
vec3 to_linear(vec3 c) {
    return mix(c / 12.92, pow((c + 0.055) / 1.055, vec3(2.4)), step(vec3(0.04045), c));
}
vec3 to_srgb(vec3 c) {
    return mix(12.92 * c, 1.055 * pow(c, vec3(1.0 / 2.4)) - 0.055, step(vec3(0.0031308), c));
}
vec4 hook() {
    vec4 c = HOOKED_tex(HOOKED_pos);
    // BT.1886 includes the target's finite black level. Both pinned SDR
    // renderers use 1000:1 contrast; treating it as a pure power curve would
    // shift native preview relative to the exported video.
    const float black = 0.056234132519;
    const float span = 1.0 - black;
    vec3 video = clamp((pow(max(to_linear(clamp(c.rgb, 0.0, 1.0)), vec3(0.0)), vec3(1.0 / 2.4)) - black) / span, 0.0, 1.0);
    float y = dot(video, vec3(0.2126, 0.7152, 0.0722));
    float adjusted = pow(max(y, 0.0), 1.0 / 0.9);
    vec3 rgb = clamp(vec3(adjusted) + 1.05 * (video - y), 0.0, 1.0);
    return vec4(to_srgb(pow(span * rgb + black, vec3(2.4))), c.a);
}
