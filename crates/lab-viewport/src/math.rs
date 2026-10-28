//! Small fixed-size vector and matrix helpers shared by drawing and picking.
pub fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(v: [f32; 3], factor: f32) -> [f32; 3] {
    [v[0] * factor, v[1] * factor, v[2] * factor]
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn normalized(v: [f32; 3]) -> Option<[f32; 3]> {
    let length = dot(v, v).sqrt();
    (length.is_finite() && length > 1e-8).then(|| scale(v, 1.0 / length))
}

pub fn multiply(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut result = [[0.0; 4]; 4];
    for (column, values) in result.iter_mut().enumerate() {
        for (row, value) in values.iter_mut().enumerate() {
            *value = (0..4).map(|index| a[index][row] * b[column][index]).sum();
        }
    }
    result
}
