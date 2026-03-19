use std::process::Command;

/// System information regarding hardware rendering and OS capabilities.
#[derive(Debug, Clone)]
pub struct SysInfo {
    pub os: String,
    pub vendor_name: String,
    pub has_nvidia: bool,
    pub has_intel: bool,
    pub has_amd: bool,
    pub has_mac_hw: bool,
    pub ffmpeg_hw_encoders: Vec<String>,
}

/// Helper function: attempt a 1-frame dummy encode using the specified encoder.
/// Prevents crash (OS Error 232) if the binary claims support but physical hw is missing.
fn test_encoder(ffmpeg_bin: &str, encoder: &str) -> bool {
    // We use a completely synthetic input (lavfi simple color)
    // and encode 1 frame using the given hardware encoder, outputting to `null` (nowhere)
    if let Ok(output) = Command::new(ffmpeg_bin)
        .arg("-y")
        .args(["-f", "lavfi"])
        .args(["-i", "color=c=black:s=16x16:r=1"])
        .args(["-vframes", "1"])
        .args(["-c:v", encoder])
        .args(["-f", "null"])
        .arg("-")
        .output()
    {
        output.status.success()
    } else {
        false
    }
}

impl SysInfo {
    /// Probe the system and FFmpeg for optimal hardware rendering paths.
    pub fn probe(ffmpeg_bin: &str) -> Self {
        let os = std::env::consts::OS.to_string();
        let mut has_nvidia = false;
        let mut has_intel = false;
        let mut has_amd = false;
        let mut has_mac_hw = false;

        // Try querying FFmpeg encoders
        let mut ffmpeg_hw_encoders = Vec::new();
        if let Ok(output) = Command::new(ffmpeg_bin).arg("-encoders").output() {
            if output.status.success() {
                let stdout = String::from_utf8_lossy(&output.stdout).to_lowercase();
                
                // h264_nvenc (NVIDIA)
                if stdout.contains("h264_nvenc") && test_encoder(ffmpeg_bin, "h264_nvenc") {
                    has_nvidia = true;
                    ffmpeg_hw_encoders.push("h264_nvenc".to_string());
                }
                
                // h264_qsv (Intel QuickSync)
                if stdout.contains("h264_qsv") && test_encoder(ffmpeg_bin, "h264_qsv") {
                    has_intel = true;
                    ffmpeg_hw_encoders.push("h264_qsv".to_string());
                }
                
                // h264_amf (AMD)
                if stdout.contains("h264_amf") && test_encoder(ffmpeg_bin, "h264_amf") {
                    has_amd = true;
                    ffmpeg_hw_encoders.push("h264_amf".to_string());
                }
                
                // hevc_videotoolbox (Mac)
                if stdout.contains("h264_videotoolbox") && test_encoder(ffmpeg_bin, "h264_videotoolbox") {
                    has_mac_hw = true;
                    ffmpeg_hw_encoders.push("h264_videotoolbox".to_string());
                }
            }
        }

        let vendor_name = if has_nvidia {
            "NVIDIA"
        } else if has_intel {
            "Intel"
        } else if has_amd {
            "AMD"
        } else if has_mac_hw {
            "Apple (VideoToolbox)"
        } else {
            "Software/CPU"
        }.to_string();

        log::info!("SysInfo Probe: OS={}, Hardware={}, FFmpeg Encoders={:?}", os, vendor_name, ffmpeg_hw_encoders);

        Self {
            os,
            vendor_name,
            has_nvidia,
            has_intel,
            has_amd,
            has_mac_hw,
            ffmpeg_hw_encoders,
        }
    }

    /// Get the best FFmpeg h264 encoder string for this system.
    pub fn best_h264_encoder(&self) -> &'static str {
        if self.has_nvidia {
            "h264_nvenc"
        } else if self.has_intel {
            "h264_qsv"
        } else if self.has_amd {
            "h264_amf"
        } else if self.has_mac_hw {
            "h264_videotoolbox"
        } else {
            "libx264" // Fallback to CPU
        }
    }
}
