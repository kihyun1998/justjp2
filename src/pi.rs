/// Packet Iterator — determines the order in which packets are written/read
/// based on the progression order (LRCP, RLCP, RPCL, PCRL, CPRL).

use crate::types::ProgOrder;

/// Minimal image parameters for packet iteration.
#[derive(Debug, Clone)]
pub struct PiImage {
    /// Number of components.
    pub num_comps: u32,
    /// Per component: number of resolution levels.
    pub num_res: Vec<u32>,
    /// [comp][res] -> (precinct_w_count, precinct_h_count).
    pub num_precincts: Vec<Vec<(u32, u32)>>,
}

/// Coding parameters for packet iteration.
#[derive(Debug, Clone)]
pub struct PiParams {
    /// Number of quality layers.
    pub num_layers: u32,
    /// Progression order.
    pub prog_order: ProgOrder,
}

/// Identifies a single packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketIndex {
    pub layer: u32,
    pub res: u32,
    pub comp: u32,
    pub precinct: u32,
}

/// Packet iterator that yields packets in the specified progression order.
pub struct PiIterator {
    #[allow(dead_code)]
    image: PiImage,
    #[allow(dead_code)]
    params: PiParams,
    packets: Vec<PacketIndex>,
    pos: usize,
}

impl PiIterator {
    /// Create iterator from image and coding parameters.
    pub fn new(image: PiImage, params: PiParams) -> Self {
        let packets = match params.prog_order {
            ProgOrder::Lrcp => generate_lrcp(&image, &params),
            ProgOrder::Rlcp => generate_rlcp(&image, &params),
            ProgOrder::Rpcl => generate_rpcl(&image, &params),
            ProgOrder::Pcrl => generate_pcrl(&image, &params),
            ProgOrder::Cprl => generate_cprl(&image, &params),
        };
        Self {
            image,
            params,
            packets,
            pos: 0,
        }
    }

    /// Total number of packets.
    pub fn packet_count(&self) -> usize {
        self.packets.len()
    }

    /// Get all packets in order.
    pub fn packets(&self) -> &[PacketIndex] {
        &self.packets
    }
}

impl Iterator for PiIterator {
    type Item = PacketIndex;

    fn next(&mut self) -> Option<PacketIndex> {
        if self.pos < self.packets.len() {
            let pkt = self.packets[self.pos];
            self.pos += 1;
            Some(pkt)
        } else {
            None
        }
    }
}

// ── Helper: total precincts for a component at a resolution ──

/// Returns the total number of precincts for component `comp` at resolution `res`.
fn total_precincts(image: &PiImage, comp: u32, res: u32) -> u32 {
    let (pw, ph) = image.num_precincts[comp as usize][res as usize];
    pw * ph
}

/// Max number of resolution levels across all components.
fn max_res(image: &PiImage) -> u32 {
    image.num_res.iter().copied().max().unwrap_or(0)
}

/// Max number of precincts at a given resolution across all components.
fn max_precincts_at_res(image: &PiImage, res: u32) -> u32 {
    let mut max_p = 0u32;
    for comp in 0..image.num_comps {
        if res < image.num_res[comp as usize] {
            max_p = max_p.max(total_precincts(image, comp, res));
        }
    }
    max_p
}

/// Max precincts across all resolutions and all components (global max).
fn global_max_precincts(image: &PiImage) -> u32 {
    let mut max_p = 0u32;
    for comp in 0..image.num_comps {
        for res in 0..image.num_res[comp as usize] {
            max_p = max_p.max(total_precincts(image, comp, res));
        }
    }
    max_p
}

// ── Progression order generators ──

/// LRCP: Layer-Resolution-Component-Precinct
fn generate_lrcp(image: &PiImage, params: &PiParams) -> Vec<PacketIndex> {
    let mut packets = Vec::new();
    let max_r = max_res(image);
    for layer in 0..params.num_layers {
        for res in 0..max_r {
            for comp in 0..image.num_comps {
                if res >= image.num_res[comp as usize] {
                    continue;
                }
                let num_p = total_precincts(image, comp, res);
                for precinct in 0..num_p {
                    packets.push(PacketIndex {
                        layer,
                        res,
                        comp,
                        precinct,
                    });
                }
            }
        }
    }
    packets
}

/// RLCP: Resolution-Layer-Component-Precinct
fn generate_rlcp(image: &PiImage, params: &PiParams) -> Vec<PacketIndex> {
    let mut packets = Vec::new();
    let max_r = max_res(image);
    for res in 0..max_r {
        for layer in 0..params.num_layers {
            for comp in 0..image.num_comps {
                if res >= image.num_res[comp as usize] {
                    continue;
                }
                let num_p = total_precincts(image, comp, res);
                for precinct in 0..num_p {
                    packets.push(PacketIndex {
                        layer,
                        res,
                        comp,
                        precinct,
                    });
                }
            }
        }
    }
    packets
}

/// RPCL: Resolution-Precinct-Component-Layer
fn generate_rpcl(image: &PiImage, params: &PiParams) -> Vec<PacketIndex> {
    let mut packets = Vec::new();
    let max_r = max_res(image);
    for res in 0..max_r {
        let max_p = max_precincts_at_res(image, res);
        for precinct in 0..max_p {
            for comp in 0..image.num_comps {
                if res >= image.num_res[comp as usize] {
                    continue;
                }
                if precinct >= total_precincts(image, comp, res) {
                    continue;
                }
                for layer in 0..params.num_layers {
                    packets.push(PacketIndex {
                        layer,
                        res,
                        comp,
                        precinct,
                    });
                }
            }
        }
    }
    packets
}

/// PCRL: Precinct-Component-Resolution-Layer
fn generate_pcrl(image: &PiImage, params: &PiParams) -> Vec<PacketIndex> {
    let mut packets = Vec::new();
    let max_p = global_max_precincts(image);
    for precinct in 0..max_p {
        for comp in 0..image.num_comps {
            let max_r = image.num_res[comp as usize];
            for res in 0..max_r {
                if precinct >= total_precincts(image, comp, res) {
                    continue;
                }
                for layer in 0..params.num_layers {
                    packets.push(PacketIndex {
                        layer,
                        res,
                        comp,
                        precinct,
                    });
                }
            }
        }
    }
    packets
}

/// CPRL: Component-Precinct-Resolution-Layer
fn generate_cprl(image: &PiImage, params: &PiParams) -> Vec<PacketIndex> {
    let mut packets = Vec::new();
    for comp in 0..image.num_comps {
        let max_r = image.num_res[comp as usize];
        // Max precincts across all resolutions for this component
        let mut max_p = 0u32;
        for res in 0..max_r {
            max_p = max_p.max(total_precincts(image, comp, res));
        }
        for precinct in 0..max_p {
            for res in 0..max_r {
                if precinct >= total_precincts(image, comp, res) {
                    continue;
                }
                for layer in 0..params.num_layers {
                    packets.push(PacketIndex {
                        layer,
                        res,
                        comp,
                        precinct,
                    });
                }
            }
        }
    }
    packets
}
