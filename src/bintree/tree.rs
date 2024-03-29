use super::{def::*, treemap::TreeMap};
use crate::{align_down, buddy::def::PAGE_SIZE};

#[derive(Debug)]
pub enum TreeErr {
    NotEnough,
    NotFound,
    WrongSize,
}

// 完全二叉树
#[repr(C)]
#[derive(Debug)]
pub struct BinTree {
    pub level: usize,          // 树的高度
    nodes: [usize; MAX_NODES], // 节点数组
    pub bitmap: TreeMap,       // 位图
}

#[allow(unused)]
impl BinTree {
    pub fn new() -> Self {
        Self {
            nodes: [0; MAX_NODES],
            bitmap: TreeMap::new(),
            level: 0,
        }
    }

    // 初始化完全二叉树
    pub fn init(&mut self, root: usize, size: usize) -> Result<usize, TreeErr> {
        let mut mem_size = align_down!(size, MIN_SIZE);
        let mut leaf_counts = mem_size / MIN_SIZE;
        let mut tmp_leaf = leaf_counts;
        let mut tmp_size = mem_size;

        if leaf_counts == 0 {
            return Err(TreeErr::NotEnough);
        }

        // 向上找到最大节点数
        while !tmp_leaf.is_power_of_two() {
            tmp_leaf += 1;
            tmp_size += PAGE_SIZE;
        }

        // 先将所有页的bit位设置为1(used)
        self.bitmap.set_bit_all();

        let node_counts = tmp_leaf * 2 - 1;
        let mut cur_size = tmp_size;
        let mut counts = 0;

        // 将页地址放入二叉树中，每放入一个则设置其bit位为0(unused)
        while counts < node_counts {
            let mut current = root;

            while current < (root + tmp_size) {
                self.nodes[counts] = current;
                self.bitmap.unset_bit(counts);

                current += cur_size;
                counts += 1;
            }

            cur_size >>= 1;
            self.level += 1;
        }

        // 将不可用的地址设为used
        if tmp_leaf > leaf_counts {
            let level = self.get_level(PAGE_SIZE);
            let idx = self.get_index(level);
            for i in (idx + leaf_counts)..(idx + tmp_leaf) {
                self.bitmap.set_bit(i);
            }
        }

        Ok(leaf_counts)
    }

    // 根据size获取对应节点位于树的高度
    pub fn get_level(&self, size: usize) -> usize {
        let mut index_size = align_down!(size, MIN_SIZE);
        let mut level = self.level;

        while index_size > MIN_SIZE {
            index_size >>= 1;
            level -= 1;
        }

        level
    }

    // 根据高度(level)获取对应节点的索引
    pub fn get_index(&self, level: usize) -> usize {
        2usize.pow((level - 1) as u32) - 1
    }

    // 根据索引获取对应节点的内容
    pub fn get_value(&self, idx: usize) -> usize {
        self.nodes[idx]
    }

    // 进行适配搜索
    // TODO
    // 目前只能找到第一个适合(used or unused)的节点，如果能返回一个迭代器或者数组
    // 也就是所有适合的节点，将更方便
    pub fn find(&self, size: usize, is_used: bool) -> Result<usize, TreeErr> {
        if size > MAX_SIZE {
            return Err(TreeErr::WrongSize);
        }

        // 寻找并检验bit位为unused的节点
        let level = self.get_level(size);
        let mut idx = self.get_index(level);

        while idx < (self.get_index(level + 1) - 1) {
            if self.bitmap.is_empty(idx) != is_used {
                let mut left_leaf = idx;

                while self.find_left_child(left_leaf) <= self.max_node() {
                    left_leaf = self.find_left_child(left_leaf);
                }

                let mut page_counts = size / MIN_SIZE;
                let mut page = 0;

                if is_used && self.can_free(left_leaf, page_counts)
                    || !is_used && self.can_use(left_leaf, page_counts)
                {
                    break;
                }
            }

            idx += 1;
        }

        if idx == self.get_index(level + 1) {
            Err(TreeErr::NotFound)
        } else {
            Ok(idx)
        }
    }

    pub fn find_match(&self, size: usize, value: usize, is_used: bool) -> Result<usize, TreeErr> {
        if size > MAX_SIZE {
            return Err(TreeErr::WrongSize);
        }

        // 找到第一个适合的节点
        // 接着遍历之后每个节点，待改进find，能够返回多个适合的节点
        let level = self.get_level(size);
        let max_idx = self.get_index(level + 1);
        let mut idx = self.find(size, is_used).unwrap();

        while idx < max_idx {
            if self.get_value(idx) == value {
                return Ok(idx);
            }
            idx += 1;
        }

        Err(TreeErr::NotFound)
    }

    // 获取树的最大节点数
    pub fn max_node(&self) -> usize {
        self.get_index(self.level + 1) - 1
    }

    // 批量设置子树的bit位为used
    pub fn use_mem(&mut self, idx: usize) {
        let mut left_leaf = idx;
        let mut level = 0;

        while left_leaf <= self.max_node() {
            for i in 0..2usize.pow(level) {
                self.bitmap.set_bit(left_leaf + i);
            }

            left_leaf = self.find_left_child(left_leaf);
            level += 1;
        }
    }

    // 批量设置子树的bit位为unused
    pub fn unuse_mem(&mut self, idx: usize) {
        let mut left_leaf = idx;
        let mut level = 0;

        while left_leaf <= self.max_node() {
            for i in 0..2usize.pow(level) {
                self.bitmap.unset_bit(left_leaf + i);
            }

            left_leaf = self.find_left_child(left_leaf);
            level += 1;
        }
    }

    // 仅设置一个bit位为used
    pub fn use_page(&mut self, idx: usize) {
        self.bitmap.set_bit(idx);
    }

    // 仅设置一个bit位为unused
    pub fn unuse_page(&mut self, idx: usize) {
        self.bitmap.unset_bit(idx);
    }

    // 找到对应节点的左孩子
    pub fn find_left_child(&self, idx: usize) -> usize {
        idx * 2 + 1
    }

    // 找到对应节点的右孩子
    pub fn find_right_child(&self, idx: usize) -> usize {
        idx * 2 + 2
    }

    // 找到对应节点的父亲
    pub fn find_parent(&self, idx: usize) -> usize {
        (idx + 1) / 2 - 1
    }

    // 批量获取对应bit位，若全为1则为1，否则为0
    pub fn can_use(&self, index: usize, counts: usize) -> bool {
        for i in 0..counts {
            if !self.bitmap.is_empty(index + i) {
                return false;
            }
        }
        true
    }

    pub fn can_free(&self, index: usize, counts: usize) -> bool {
        for i in 0..counts {
            if self.bitmap.is_empty(index + i) {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
pub mod tests {
    use super::BinTree;
    use crate::def::PGSZ;
    extern crate alloc;
    extern crate std;
    use std::println;
    use xxos_log::{info, init_log, WriteLog};
    struct PT;

    impl WriteLog for PT {
        fn print(&self, log_content: core::fmt::Arguments) {
            println!("{}", log_content);
        }
    }

    #[test]
    fn get_level_test() {
        let mut tree1 = BinTree::new();
        let mut tree2 = BinTree::new();
        let mut tree3 = BinTree::new();
        let _ = tree1.init(0x10000, PGSZ);
        let _ = tree2.init(0x10000, PGSZ * 2);
        let _ = tree3.init(0x10000, PGSZ * 3);

        for i in 0..tree1.level {
            assert_eq!(i + 1, tree1.get_level(PGSZ * (1 >> i)));
        }

        for i in 0..tree2.level {
            assert_eq!(i + 1, tree2.get_level(PGSZ * (2 >> i)));
        }

        for i in 0..tree3.level {
            assert_eq!(i + 1, tree3.get_level(PGSZ * (2 >> i)));
        }
    }

    #[test]
    fn get_index_test() {
        let mut tree1 = BinTree::new();
        let mut tree2 = BinTree::new();
        let mut tree3 = BinTree::new();
        let _ = tree1.init(0x10000, PGSZ);
        let _ = tree2.init(0x10000, PGSZ * 2);
        let _ = tree3.init(0x10000, PGSZ * 3);

        for i in 0..tree1.level {
            assert_eq!((2usize.pow(i as u32)) - 1, tree1.get_index(i + 1));
        }

        for i in 0..tree2.level {
            assert_eq!((2usize.pow(i as u32)) - 1, tree2.get_index(i + 1));
        }

        for i in 0..tree3.level {
            assert_eq!((2usize.pow(i as u32)) - 1, tree3.get_index(i + 1));
        }
    }

    #[test]
    fn find_test() {
        let mut tree = BinTree::new();
        let _ = tree.init(0x10000, PGSZ << 1);

        assert!(tree.find(PGSZ << 1, false).is_ok());
        assert_eq!(0, tree.find(PGSZ << 1, false).unwrap());
        assert!(tree.find(PGSZ, false).is_ok());
        assert_eq!(1, tree.find(PGSZ, false).unwrap());
        tree.bitmap.set_bit(1);
        assert!(tree.find(PGSZ, false).is_ok());
        assert_eq!(2, tree.find(PGSZ, false).unwrap());
        assert!(tree.find(PGSZ, true).is_ok());
        assert_eq!(1, tree.find(PGSZ, true).unwrap());
    }

    #[test]
    fn init_test() {
        init_log(&PT, xxos_log::Level::INFO);

        let mut tree = BinTree::new();
        let mut bad_tree = BinTree::new();

        let gen_success = tree.init(0x10000, PGSZ * 10);
        let gen_error = bad_tree.init(0x10000, PGSZ / 2);

        assert!(gen_success.is_ok());
        assert!(gen_error.is_err());

        info!("{:x?}", tree);
    }
}
