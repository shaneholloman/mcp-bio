use super::*;

mod construction;
mod parsing;

fn float32_chunk(values: &[f32]) -> alphagenome_proto::TensorChunk {
    let mut data = Vec::with_capacity(values.len() * 4);
    for value in values {
        data.extend_from_slice(&value.to_le_bytes());
    }
    alphagenome_proto::TensorChunk {
        data,
        compression_type: 0,
    }
}
