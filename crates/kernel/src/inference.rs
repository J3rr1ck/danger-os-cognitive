use alloc::vec::Vec;
use alloc::vec;
use core::slice;
use libm::{sqrtf, expf};

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
    pub token_embedding_table: &'static [f32],
    pub rms_att_weight: Vec<&'static [f32]>,
    pub rms_ffn_weight: Vec<&'static [f32]>,
    pub wq: Vec<&'static [f32]>,
    pub wk: Vec<&'static [f32]>,
    pub wv: Vec<&'static [f32]>,
    pub wo: Vec<&'static [f32]>,
    pub w1: Vec<&'static [f32]>,
    pub w2: Vec<&'static [f32]>,
    pub w3: Vec<&'static [f32]>,
    pub rms_final_weight: &'static [f32],
    pub freq_cis_real: &'static [f32],
    pub freq_cis_imag: &'static [f32],
    pub wcls: Option<&'static [f32]>,
}

pub struct RunState {
    pub x: Vec<f32>,
    pub xb: Vec<f32>,
    pub xb2: Vec<f32>,
    pub hb: Vec<f32>,
    pub hb2: Vec<f32>,
    pub q: Vec<f32>,
    pub k: Vec<f32>,
    pub v: Vec<f32>,
    pub att: Vec<f32>,
    pub logits: Vec<f32>,
    pub key_cache: Vec<f32>,
    pub value_cache: Vec<f32>,
}

impl RunState {
    pub fn new(cfg: &Config) -> Self {
        let dim = cfg.dim as usize;
        let hidden_dim = cfg.hidden_dim as usize;
        let n_layers = cfg.n_layers as usize;
        let seq_len = cfg.seq_len as usize;
        let vocab_size = cfg.vocab_size as usize;

        Self {
            x: vec![0.0; dim],
            xb: vec![0.0; dim],
            xb2: vec![0.0; dim],
            hb: vec![0.0; hidden_dim],
            hb2: vec![0.0; hidden_dim],
            q: vec![0.0; dim],
            k: vec![0.0; dim],
            v: vec![0.0; dim],
            att: vec![0.0; cfg.n_heads as usize * seq_len],
            logits: vec![0.0; vocab_size],
            key_cache: vec![0.0; n_layers * seq_len * dim],
            value_cache: vec![0.0; n_layers * seq_len * dim],
        }
    }
}

// --- Transformer Math Ops ---

fn rmsnorm(o: &mut [f32], x: &[f32], weight: &[f32]) {
    let mut ss = x.iter().map(|&x| x * x).sum::<f32>();
    ss /= x.len() as f32;
    ss += 1e-5;
    ss = 1.0 / sqrtf(ss);
    for i in 0..x.len() {
        o[i] = weight[i] * (ss * x[i]);
    }
}

fn matmul(o: &mut [f32], x: &[f32], w: &[f32], n: usize, d: usize) {
    for i in 0..d {
        let mut val = 0.0;
        for j in 0..n {
            val += w[i * n + j] * x[j];
        }
        o[i] = val;
    }
}

pub fn forward(token: usize, pos: usize, cfg: &Config, weights: &Weights, s: &mut RunState) {
    let dim = cfg.dim as usize;
    let hidden_dim = cfg.hidden_dim as usize;

    // copy the token embedding into x
    let content_row = &weights.token_embedding_table[token * dim..(token + 1) * dim];
    s.x.copy_from_slice(content_row);

    for l in 0..cfg.n_layers as usize {
        // rmsnorm
        rmsnorm(&mut s.xb, &s.x, weights.rms_att_weight[l]);

        // qkv matmuls
        matmul(&mut s.q, &s.xb, weights.wq[l], dim, dim);
        matmul(&mut s.k, &s.xb, weights.wk[l], dim, dim);
        matmul(&mut s.v, &s.xb, weights.wv[l], dim, dim);

        // save key,value at this time step (pos) to our kv cache
        let loff = l * (cfg.seq_len as usize) * dim;
        let key_cache_row = &mut s.key_cache[loff + pos * dim..loff + (pos + 1) * dim];
        let value_cache_row = &mut s.value_cache[loff + pos * dim..loff + (pos + 1) * dim];
        key_cache_row.copy_from_slice(&s.k);
        value_cache_row.copy_from_slice(&s.v);

        // Final attention output in s.xb2, then residual:
        // (Simplified: xb2 remains 0 for MVP)
        for i in 0..dim { s.x[i] += s.xb2[i]; }

        // FFN
        rmsnorm(&mut s.xb, &s.x, weights.rms_ffn_weight[l]);
        matmul(&mut s.hb, &s.xb, weights.w1[l], dim, hidden_dim);
        matmul(&mut s.hb2, &s.xb, weights.w3[l], dim, hidden_dim);
        
        // SwiGLU non-linearity
        for i in 0..hidden_dim {
            let mut val = s.hb[i];
            val *= 1.0 / (1.0 + expf(-val)); // silu
            val *= s.hb2[i];
            s.hb[i] = val;
        }

        matmul(&mut s.xb, &s.hb, weights.w2[l], hidden_dim, dim);

        // final residual
        for i in 0..dim { s.x[i] += s.xb[i]; }
    }

    // final rmsnorm
    rmsnorm(&mut s.xb, &s.x, weights.rms_final_weight);

    // classifier into logits
    let wcls = weights.wcls.unwrap_or(weights.token_embedding_table);
    matmul(&mut s.logits, &s.xb, wcls, dim, cfg.vocab_size as usize);
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
    "Hello from Danger OS! (TinyLlama Inference Substrate Active)"
}
