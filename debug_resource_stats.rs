// Add this to CanvasApp struct for debugging resource usage
#[derive(Clone)]
struct ResourceStats {
    total_textures: usize,
    total_blocks: usize,
    total_frames: usize,
    memory_estimate_mb: f64,
}

impl Default for ResourceStats {
    fn default() -> Self {
        Self {
            total_textures: 0,
            total_blocks: 0,
            total_frames: 0,
            memory_estimate_mb: 0.0,
        }
    }
}

// Add this method to CanvasApp
impl CanvasApp {
    fn calculate_resource_stats(&self) -> ResourceStats {
        let mut stats = ResourceStats::default();
        
        stats.total_blocks = self.blocks.len();
        
        for block in &self.blocks {
            if let BlockContent::Image { frames, .. } = &block.content {
                stats.total_textures += frames.len();
                stats.total_frames += frames.len();
                
                // Estimate memory usage (rough calculation)
                for texture in frames {
                    if let Some(size) = texture.size_vec2() {
                        let pixels = (size.x * size.y) as f64;
                        // RGBA = 4 bytes per pixel
                        let memory_mb = (pixels * 4.0) / (1024.0 * 1024.0);
                        stats.memory_estimate_mb += memory_mb;
                    }
                }
            }
        }
        
        stats
    }
    
    fn log_resource_stats(&self) {
        let stats = self.calculate_resource_stats();
        eprintln!("=== Resource Stats ===");
        eprintln!("Blocks: {}", stats.total_blocks);
        eprintln!("Textures: {}", stats.total_textures);
        eprintln!("Frames: {}", stats.total_frames);
        eprintln!("Estimated Memory: {:.2} MB", stats.memory_estimate_mb);
        eprintln!("====================");
    }
}