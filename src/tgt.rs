/// 태그 트리 (Tag Tree)
///
/// 코드블록의 inclusion / zero-bitplane 정보를 계층적으로 인코딩/디코딩하는 트리.
/// 리프 노드에서 루트까지 ceil 기반으로 부모 노드를 생성한다.

use crate::bio::{BioReader, BioWriter};
use crate::error::Result;

/// 태그 트리 노드
#[derive(Debug, Clone)]
pub struct TgtNode {
    /// 노드의 값
    pub value: i32,
    /// 현재까지 알려진 하한
    pub low: i32,
    /// 값이 확정되었는지 여부
    pub known: bool,
}

impl Default for TgtNode {
    fn default() -> Self {
        Self {
            value: 0,
            low: 0,
            known: false,
        }
    }
}

/// 태그 트리
#[derive(Debug)]
pub struct TgtTree {
    /// 리프 수 (가로)
    pub num_leafs_h: u32,
    /// 리프 수 (세로)
    pub num_leafs_v: u32,
    /// 전체 노드 배열 (레벨별 연속 저장)
    pub nodes: Vec<TgtNode>,
    /// 각 레벨의 시작 인덱스와 (w, h)
    levels: Vec<(usize, u32, u32)>,
}

impl TgtTree {
    /// 새 태그 트리 생성
    pub fn create(num_leafs_h: u32, num_leafs_v: u32) -> Self {
        let mut levels = Vec::new();
        let mut total_nodes = 0usize;
        let mut w = num_leafs_h;
        let mut h = num_leafs_v;

        loop {
            levels.push((total_nodes, w, h));
            total_nodes += (w as usize) * (h as usize);
            if w == 1 && h == 1 {
                break;
            }
            w = (w + 1) / 2;
            h = (h + 1) / 2;
        }

        let nodes = vec![TgtNode::default(); total_nodes];

        Self {
            num_leafs_h,
            num_leafs_v,
            nodes,
            levels,
        }
    }

    /// 전체 노드 수
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// 레벨 수
    pub fn num_levels(&self) -> usize {
        self.levels.len()
    }

    /// 트리 상태 리셋
    pub fn reset(&mut self) {
        for node in &mut self.nodes {
            node.low = 0;
            node.known = false;
        }
    }

    /// 리프 노드의 값을 설정
    pub fn setvalue(&mut self, leaf_h: u32, leaf_v: u32, value: i32) {
        let idx = self.node_index(0, leaf_h, leaf_v);
        self.nodes[idx].value = value;

        // 부모 노드 값 갱신 (최소값 전파)
        let mut lh = leaf_h;
        let mut lv = leaf_v;
        for level in 1..self.levels.len() {
            let ph = lh / 2;
            let pv = lv / 2;
            let parent_idx = self.node_index(level, ph, pv);

            // 부모의 값 = 자식들의 최소값
            let (start, w, _h) = self.levels[level - 1];
            let child_w = self.levels[level - 1].1;
            let child_h = self.levels[level - 1].2;

            let c0h = ph * 2;
            let c0v = pv * 2;
            let mut min_val = i32::MAX;
            for cv in c0v..std::cmp::min(c0v + 2, child_h) {
                for ch in c0h..std::cmp::min(c0h + 2, child_w) {
                    let ci = start + (cv as usize) * (w as usize) + (ch as usize);
                    min_val = std::cmp::min(min_val, self.nodes[ci].value);
                }
            }
            self.nodes[parent_idx].value = min_val;

            lh = ph;
            lv = pv;
        }
    }

    /// 태그 트리 인코딩
    pub fn encode(&mut self, bio: &mut BioWriter, leaf_h: u32, leaf_v: u32, threshold: i32) {
        // 루트에서 리프까지의 경로를 수집
        let path = self.collect_path(leaf_h, leaf_v);

        // 루트→리프 순으로 인코딩
        let mut low = 0i32;
        for &idx in &path {
            let node = &self.nodes[idx];
            if node.known {
                low = node.value;
                continue;
            }

            let node_low = std::cmp::max(low, node.low);
            let node_value = node.value;

            let mut current = node_low;
            while current < threshold {
                if current >= node_value {
                    // 값에 도달: 1 출력
                    bio.putbit(1);
                    let node = &mut self.nodes[idx];
                    node.known = true;
                    node.low = node_value;
                    low = node_value;
                    break;
                } else {
                    // 아직 아님: 0 출력
                    bio.putbit(0);
                    current += 1;
                }
            }

            if !self.nodes[idx].known {
                self.nodes[idx].low = current;
                low = current;
            }
        }
    }

    /// 태그 트리 디코딩
    pub fn decode(
        &mut self,
        bio: &mut BioReader,
        leaf_h: u32,
        leaf_v: u32,
        threshold: i32,
    ) -> Result<i32> {
        let path = self.collect_path(leaf_h, leaf_v);

        let mut low = 0i32;
        for &idx in &path {
            let node = &self.nodes[idx];
            if node.known {
                low = node.value;
                continue;
            }

            let node_low = std::cmp::max(low, node.low);

            let mut current = node_low;
            while current < threshold {
                let bit = bio.getbit()?;
                if bit == 1 {
                    // 값 확정
                    let node = &mut self.nodes[idx];
                    node.value = current;
                    node.known = true;
                    node.low = current;
                    low = current;
                    break;
                }
                current += 1;
            }

            if !self.nodes[idx].known {
                self.nodes[idx].low = current;
                low = current;
            }
        }

        Ok(self.nodes[self.node_index(0, leaf_h, leaf_v)].value)
    }

    /// 레벨과 좌표로 노드 인덱스 계산
    fn node_index(&self, level: usize, h: u32, v: u32) -> usize {
        let (start, w, _) = self.levels[level];
        start + (v as usize) * (w as usize) + (h as usize)
    }

    /// 루트→리프 경로의 인덱스 수집
    fn collect_path(&self, leaf_h: u32, leaf_v: u32) -> Vec<usize> {
        let mut path = Vec::with_capacity(self.levels.len());
        let mut h = leaf_h;
        let mut v = leaf_v;

        // 리프(level 0)에서 루트까지 수집
        for level in 0..self.levels.len() {
            path.push(self.node_index(level, h, v));
            h /= 2;
            v /= 2;
        }

        // 루트→리프 순서로 뒤집기
        path.reverse();
        path
    }
}
