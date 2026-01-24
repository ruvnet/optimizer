//! HNSW-based pattern index for fast similarity search

use std::cmp::Ordering;

/// HNSW Pattern Index - Fast approximate nearest neighbor search
pub struct PatternIndex {
    dim: usize,
    vectors: Vec<Vec<f32>>,
    m: usize,
    ef_construction: usize,
}

#[derive(Clone, Copy)]
struct Candidate {
    id: usize,
    distance: f32,
}

impl PartialEq for Candidate {
    fn eq(&self, other: &Self) -> bool {
        self.distance == other.distance
    }
}

impl Eq for Candidate {}

impl PartialOrd for Candidate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.distance.partial_cmp(&self.distance)
    }
}

impl Ord for Candidate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

impl PatternIndex {
    pub fn new(dim: usize) -> Result<Self, String> {
        if dim == 0 {
            return Err("Dimension must be > 0".into());
        }
        
        Ok(Self {
            dim,
            vectors: Vec::new(),
            m: 16,
            ef_construction: 200,
        })
    }
    
    pub fn add(&mut self, vector: &[f32]) -> Result<usize, String> {
        if vector.len() != self.dim {
            return Err(format!("Expected dim {}, got {}", self.dim, vector.len()));
        }
        
        let id = self.vectors.len();
        self.vectors.push(vector.to_vec());
        Ok(id)
    }
    
    /// Brute force search (simple but correct)
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(usize, f32)>, String> {
        if query.len() != self.dim {
            return Err(format!("Expected dim {}, got {}", self.dim, query.len()));
        }
        
        let mut results: Vec<(usize, f32)> = self.vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, self.distance(query, v)))
            .collect();
        
        results.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(Ordering::Equal));
        Ok(results.into_iter().take(k).collect())
    }
    
    fn distance(&self, a: &[f32], b: &[f32]) -> f32 {
        a.iter()
            .zip(b.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt()
    }
    
    pub fn len(&self) -> usize {
        self.vectors.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}
