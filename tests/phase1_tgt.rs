use justjp2::bio::{BioReader, BioWriter};
use justjp2::tgt::TgtTree;

#[test]
fn create_1x1() {
    let tree = TgtTree::create(1, 1);
    assert_eq!(tree.num_nodes(), 1);
    assert_eq!(tree.num_levels(), 1);
}

#[test]
fn create_4x4() {
    let tree = TgtTree::create(4, 4);
    // level 0: 4x4=16, level 1: 2x2=4, level 2: 1x1=1 → 21
    assert_eq!(tree.num_nodes(), 21);
    assert_eq!(tree.num_levels(), 3);
}

#[test]
fn create_3x2() {
    let tree = TgtTree::create(3, 2);
    // level 0: 3x2=6, level 1: 2x1=2, level 2: 1x1=1 → 9
    assert_eq!(tree.num_nodes(), 9);
    assert_eq!(tree.num_levels(), 3);
}

#[test]
fn setvalue_and_read() {
    let mut tree = TgtTree::create(2, 2);
    tree.setvalue(0, 0, 5);
    tree.setvalue(1, 0, 3);
    tree.setvalue(0, 1, 7);
    tree.setvalue(1, 1, 2);

    // 리프 노드 값 확인
    assert_eq!(tree.nodes[0].value, 5);
    assert_eq!(tree.nodes[1].value, 3);
    assert_eq!(tree.nodes[2].value, 7);
    assert_eq!(tree.nodes[3].value, 2);

    // 루트(부모)는 최소값 = 2
    assert_eq!(tree.nodes[4].value, 2);
}

#[test]
fn reset_clears_state() {
    let mut tree = TgtTree::create(2, 2);
    tree.setvalue(0, 0, 5);

    // 인코딩으로 known 상태 변경
    let mut bio = BioWriter::new();
    tree.encode(&mut bio, 0, 0, 10);

    // known 상태 확인
    assert!(tree.nodes.iter().any(|n| n.known));

    // reset
    tree.reset();
    assert!(tree.nodes.iter().all(|n| !n.known));
    assert!(tree.nodes.iter().all(|n| n.low == 0));
}

#[test]
fn encode_decode_roundtrip() {
    let mut enc_tree = TgtTree::create(2, 2);
    enc_tree.setvalue(0, 0, 3);
    enc_tree.setvalue(1, 0, 5);
    enc_tree.setvalue(0, 1, 2);
    enc_tree.setvalue(1, 1, 4);

    let threshold = 10;

    // 인코딩
    let mut bio_w = BioWriter::new();
    enc_tree.encode(&mut bio_w, 0, 0, threshold);
    enc_tree.encode(&mut bio_w, 1, 0, threshold);
    enc_tree.encode(&mut bio_w, 0, 1, threshold);
    enc_tree.encode(&mut bio_w, 1, 1, threshold);
    bio_w.flush().unwrap();

    // 디코딩
    let data = bio_w.into_vec();
    let mut bio_r = BioReader::new(&data);
    let mut dec_tree = TgtTree::create(2, 2);

    let v0 = dec_tree.decode(&mut bio_r, 0, 0, threshold).unwrap();
    let v1 = dec_tree.decode(&mut bio_r, 1, 0, threshold).unwrap();
    let v2 = dec_tree.decode(&mut bio_r, 0, 1, threshold).unwrap();
    let v3 = dec_tree.decode(&mut bio_r, 1, 1, threshold).unwrap();

    assert_eq!(v0, 3);
    assert_eq!(v1, 5);
    assert_eq!(v2, 2);
    assert_eq!(v3, 4);
}

#[test]
fn encode_threshold() {
    let mut tree = TgtTree::create(1, 1);
    tree.setvalue(0, 0, 5);

    // threshold=3이면 값 5는 인코딩되지 않음 (known=false)
    let mut bio_w = BioWriter::new();
    tree.encode(&mut bio_w, 0, 0, 3);
    assert!(!tree.nodes[0].known);

    // threshold=6이면 값 5가 인코딩됨
    tree.reset();
    let mut bio_w2 = BioWriter::new();
    tree.encode(&mut bio_w2, 0, 0, 6);
    assert!(tree.nodes[0].known);
}

#[test]
fn multi_leaf_encode_decode() {
    let mut enc_tree = TgtTree::create(4, 4);
    let values = [
        [3, 1, 4, 1],
        [5, 9, 2, 6],
        [5, 3, 5, 8],
        [9, 7, 9, 3],
    ];

    for v in 0..4u32 {
        for h in 0..4u32 {
            enc_tree.setvalue(h, v, values[v as usize][h as usize]);
        }
    }

    let threshold = 15;
    let mut bio_w = BioWriter::new();
    for v in 0..4u32 {
        for h in 0..4u32 {
            enc_tree.encode(&mut bio_w, h, v, threshold);
        }
    }
    bio_w.flush().unwrap();

    let data = bio_w.into_vec();
    let mut bio_r = BioReader::new(&data);
    let mut dec_tree = TgtTree::create(4, 4);

    for v in 0..4u32 {
        for h in 0..4u32 {
            let decoded = dec_tree.decode(&mut bio_r, h, v, threshold).unwrap();
            assert_eq!(
                decoded, values[v as usize][h as usize],
                "mismatch at ({h}, {v})"
            );
        }
    }
}

#[test]
fn parent_propagation() {
    let mut tree = TgtTree::create(4, 4);
    // 전부 10으로 설정
    for v in 0..4u32 {
        for h in 0..4u32 {
            tree.setvalue(h, v, 10);
        }
    }
    // (0,0)만 2로 변경
    tree.setvalue(0, 0, 2);

    // level 1의 (0,0) 부모는 min(2, 10, 10, 10) = 2
    // 루트도 min(2, ...) = 2
    let root_idx = tree.num_nodes() - 1;
    assert_eq!(tree.nodes[root_idx].value, 2);
}
