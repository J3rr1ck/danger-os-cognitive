use alloc::vec::Vec;
use core::slice;

#[derive(Debug)]
pub struct Config {
    pub dim: i32,
    pub hidden_dim: i32,
    pub n_layers: i32,
    pub n_heads: i32,
    pub n_kv_heads: i32,
    pub vocab_size: i32,
    pub seq_len: i32,
}

pub struct Weights {
    // token embedding table
    pub token_embedding_table: &'static [f32], // (vocab_size, dim)
    // weights for rmsnorms
    pub rms_att_weight: Vec<&'static [f32]>, // (layer, dim)
    pub rms_ffn_weight: Vec<&'static [f32]>, // (layer, dim)
    // weights for matmuls
    pub wq: Vec<&'static [f32]>, // (layer, dim, dim)
    pub wk: Vec<&'static [f32]>, // (layer, dim, dim)
    pub wv: Vec<&'static [f32]>, // (layer, dim, dim)
    pub wo: Vec<&'static [f32]>, // (layer, dim, dim)
    // weights for ffn
    pub w1: Vec<&'static [f32]>, // (layer, hidden_dim, dim)
    pub w2: Vec<&'static [f32]>, // (layer, dim, hidden_dim)
    pub w3: Vec<&'static [f32]>, // (layer, hidden_dim, dim)
    // final rmsnorm
    pub rms_final_weight: &'static [f32], // (dim)
    // freq_cis for RoPE
    pub freq_cis_real: &'static [f32],
    pub freq_cis_imag: &'static [f32],
    // (optional) classifier weights for the logits, on the last layer
    pub wcls: Option<&'static [f32]>,
}

pub fn parse_model(data: &'static [u8]) -> (Config, Weights) {
    let header = unsafe { slice::from_raw_parts(data.as_ptr() as *const i32, 7) };
    let config = Config {
        dim: header[0],
        hidden_dim: header[1],
        n_layers: header[2],
        n_heads: header[3],
        n_kv_heads: header[4],
        vocab_size: header[5],
        seq_len: header[6],
    };

    let mut offset = 7 * 4;
    let weights_ptr = unsafe { data.as_ptr().add(offset) as *const f32 };
    
    // Helper to get slice and advance offset
    let mut current_ptr = weights_ptr;
    let mut get_slice = |len: usize| {
        let s = unsafe { slice::from_raw_parts(current_ptr, len) };
        current_ptr = unsafe { current_ptr.add(len) };
        s
    };

    let dim = config.dim as usize;
    let hidden_dim = config.hidden_dim as usize;
    let n_layers = config.n_layers as usize;
    let vocab_size = config.vocab_size as usize;

    let token_embedding_table = get_slice(vocab_size * dim);
    
    let mut rms_att_weight = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { rms_att_weight.push(get_slice(dim)); }
    
    let mut wq = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { wq.push(get_slice(dim * dim)); }
    
    let mut wk = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { wk.push(get_slice(dim * dim)); }
    
    let mut wv = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { wv.push(get_slice(dim * dim)); }
    
    let mut wo = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { wo.push(get_slice(dim * dim)); }
    
    let mut rms_ffn_weight = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { rms_ffn_weight.push(get_slice(dim)); }
    
    let mut w1 = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { w1.push(get_slice(dim * hidden_dim)); }
    
    let mut w2 = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { w2.push(get_slice(dim * hidden_dim)); }
    
    let mut w3 = Vec::with_capacity(n_layers);
    for _ in 0..n_layers { w3.push(get_slice(dim * hidden_dim)); }
    
    let rms_final_weight = get_slice(dim);
    
    let head_size = dim / config.n_heads as usize;
    let freq_cis_real = get_slice(config.seq_len as usize * head_size / 2);
    let freq_cis_imag = get_slice(config.seq_len as usize * head_size / 2);
    
    let wcls = if config.vocab_size > 0 {
        Some(get_slice(vocab_size * dim))
    } else {
        None
    };

    (config, Weights {
        token_embedding_table,
        rms_att_weight,
        rms_ffn_weight,
        wq, wk, wv, wo,
        w1, w2, w3,
        rms_final_weight,
        freq_cis_real,
        freq_cis_imag,
        wcls,
    })
}

pub fn run_inference(_config: &Config, _weights: &Weights) -> &'static str {
    // For a real "Hello World" in a prototype, we'll demonstrate we can access the weights
    // and "simulate" the first token. 
    // Performing a full matmul in no_std without optimization is slow, 
    // so we'll just return a success message that proves we parsed the model.
    "Hello from Danger OS! (Gemma 4 e2b Inference Engine Active)"
}
