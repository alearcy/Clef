use crate::protocol::{BLOCK, Frame, RATE, Result};
use std::collections::VecDeque;

#[derive(Clone)]
pub enum Kind {
    Input,
    Sum,
    Delay(u32),
}

#[derive(Clone)]
pub struct Node {
    pub kind: Kind,
    pub inputs: Vec<usize>,
    pub channels: u8,
}

pub(crate) struct Delay(VecDeque<Frame>);
impl Delay {
    pub(crate) fn new(samples: u32) -> Self {
        Self(std::iter::repeat_n([0.0; 2], samples as usize).collect())
    }
    pub(crate) fn apply(&mut self, frame: Frame) -> Frame {
        if self.0.is_empty() {
            return frame;
        }
        let delayed = self.0.pop_front().expect("nonempty delay");
        self.0.push_back(frame);
        delayed
    }
}

struct Edge {
    source: usize,
    delay: Delay,
}

pub struct Plan {
    order: Vec<usize>,
    nodes: Vec<Node>,
    edges: Vec<Vec<Edge>>,
    delays: Vec<Delay>,
    buffers: Vec<[Frame; BLOCK]>,
    pub latency: u32,
}

impl Plan {
    pub fn compile(nodes: Vec<Node>) -> Result<Self> {
        if nodes.is_empty() || nodes.len() > 32 {
            return Err("routing plan must have 1..32 nodes".into());
        }
        for (i, node) in nodes.iter().enumerate() {
            if node.channels != 2
                || node
                    .inputs
                    .iter()
                    .any(|&source| source >= nodes.len() || source == i)
            {
                return Err("invalid stereo routing connection".into());
            }
            match node.kind {
                Kind::Input if !node.inputs.is_empty() => {
                    return Err("input node cannot have incoming edges".into());
                }
                Kind::Delay(samples) if node.inputs.len() != 1 || samples > RATE * 10 => {
                    return Err("invalid delay node".into());
                }
                Kind::Sum if node.inputs.is_empty() => {
                    return Err("sum node requires an input".into());
                }
                _ => {}
            }
        }
        let mut order = Vec::new();
        while order.len() < nodes.len() {
            let next = (0..nodes.len()).find(|i| {
                !order.contains(i) && nodes[*i].inputs.iter().all(|source| order.contains(source))
            });
            match next {
                Some(i) => order.push(i),
                None => return Err("routing cycle rejected".into()),
            }
        }
        let mut latencies = vec![0_u32; nodes.len()];
        let mut edges: Vec<Vec<Edge>> = (0..nodes.len()).map(|_| Vec::new()).collect();
        let mut delays: Vec<_> = (0..nodes.len()).map(|_| Delay::new(0)).collect();
        for &index in &order {
            let node = &nodes[index];
            let incoming = node
                .inputs
                .iter()
                .map(|&source| latencies[source])
                .max()
                .unwrap_or(0);
            for &source in &node.inputs {
                edges[index].push(Edge {
                    source,
                    delay: Delay::new(incoming - latencies[source]),
                });
            }
            let own = if let Kind::Delay(samples) = node.kind {
                samples
            } else {
                0
            };
            latencies[index] = incoming
                .checked_add(own)
                .ok_or("routing latency overflow")?;
            delays[index] = Delay::new(own);
        }
        let latency = *latencies.last().ok_or("empty routing plan")?;
        let buffers = vec![[[0.0; 2]; BLOCK]; nodes.len()];
        Ok(Self {
            order,
            nodes,
            edges,
            delays,
            buffers,
            latency,
        })
    }

    pub fn reset(&mut self) -> Result<()> {
        *self = Self::compile(self.nodes.clone())?;
        Ok(())
    }

    pub fn direct() -> Self {
        Self::compile(vec![Node {
            kind: Kind::Input,
            inputs: Vec::new(),
            channels: 2,
        }])
        .expect("valid direct plan")
    }

    pub fn parallel() -> Self {
        Self::compile(vec![
            Node {
                kind: Kind::Input,
                inputs: vec![],
                channels: 2,
            },
            Node {
                kind: Kind::Delay(257),
                inputs: vec![0],
                channels: 2,
            },
            Node {
                kind: Kind::Sum,
                inputs: vec![0, 1],
                channels: 2,
            },
        ])
        .expect("valid fixture plan")
    }

    pub fn process(&mut self, input: &[Frame]) -> Result<&[Frame]> {
        if input.is_empty() || input.len() > BLOCK {
            return Err("invalid graph block size".into());
        }
        for &index in &self.order {
            if matches!(self.nodes[index].kind, Kind::Input) {
                self.buffers[index][..input.len()].copy_from_slice(input);
                continue;
            }
            let mut sum = [[0.0_f32; 2]; BLOCK];
            for edge in &mut self.edges[index] {
                for (i, mixed) in sum[..input.len()].iter_mut().enumerate() {
                    let frame = edge.delay.apply(self.buffers[edge.source][i]);
                    mixed[0] += frame[0];
                    mixed[1] += frame[1];
                }
            }
            for (i, frame) in sum[..input.len()].iter().enumerate() {
                self.buffers[index][i] = self.delays[index].apply(*frame);
            }
        }
        Ok(&self.buffers.last().ok_or("empty graph")?[..input.len()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pdc_aligns_unequal_paths_at_the_same_sample() {
        let mut plan = Plan::parallel();
        let mut rendered = Vec::new();
        for block in 0..5 {
            let mut input = [[0.0; 2]; BLOCK];
            if block == 0 {
                input[0] = [0.25; 2];
            }
            rendered.extend_from_slice(plan.process(&input).unwrap());
        }
        assert_eq!(plan.latency, 257);
        assert_eq!(rendered[257], [0.5; 2]);
        assert!(
            rendered
                .iter()
                .enumerate()
                .all(|(i, frame)| i == 257 || *frame == [0.0; 2])
        );
    }
    #[test]
    fn cycles_and_channel_mismatches_do_not_replace_a_valid_plan() {
        let mut current = Plan::direct();
        let invalid = vec![
            Node {
                kind: Kind::Sum,
                inputs: vec![1],
                channels: 2,
            },
            Node {
                kind: Kind::Sum,
                inputs: vec![0],
                channels: 2,
            },
        ];
        assert!(Plan::compile(invalid).is_err());
        assert!(
            Plan::compile(vec![Node {
                kind: Kind::Input,
                inputs: vec![],
                channels: 1
            }])
            .is_err()
        );
        assert_eq!(current.process(&[[0.2; 2]]).unwrap(), &[[0.2; 2]]);
    }
    #[test]
    fn compensation_is_independent_of_driver_block_partition() {
        let render = |chunk: usize| {
            let mut plan = Plan::parallel();
            let mut output = Vec::new();
            let mut input = vec![[0.0; 2]; 800];
            input[37] = [0.1; 2];
            for block in input.chunks(chunk) {
                output.extend_from_slice(plan.process(block).unwrap());
            }
            output
        };
        assert_eq!(render(128), render(73));
    }
}
