//! Polygon triangulation by ear clipping (mapbox/earcut, as three.js ships
//! it). The doubly linked rings live in one `Vec<Node>`; links are indices
//! and -1 is "none".

use ranger::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Node {
    pub i: i64,
    pub x: f64,
    pub y: f64,
    pub prev: i64,
    pub next: i64,
    pub z: i64,
    pub prev_z: i64,
    pub next_z: i64,
    pub steiner: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Earcut {
    pub nodes: Vec<Node>,
    pub triangles: Vec<i64>,
}

/// Triangle vertex indices (three per triangle) for the flat 2D outline
/// `data` whose holes start at the vertex indices `hole_indices`.
pub fn triangulate(data: &Vec<f64>, hole_indices: &Vec<i64>) -> Vec<i64> {
    let mut e = Earcut { nodes: Vec::new(), triangles: Vec::new() };
    let dim: i64 = 2;
    let has_holes = !hole_indices.is_empty();
    let outer_len = if has_holes { hole_indices[0] * dim } else { data.len() as i64 };
    let mut outer = e.linked_list(data, 0, outer_len, dim, true);
    if outer < 0 || e.nx(outer) == e.pv(outer) {
        return e.triangles;
    }
    if has_holes {
        outer = e.eliminate_holes(data, hole_indices, outer, dim);
    }
    let mut min_x = 0.0;
    let mut min_y = 0.0;
    let mut inv_size = 0.0;
    if data.len() as i64 > 80 * dim {
        min_x = data[0];
        min_y = data[1];
        let mut max_x = min_x;
        let mut max_y = min_y;
        let mut i = dim;
        while i < outer_len {
            let x = data[i as usize];
            let y = data[(i + 1) as usize];
            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x > max_x {
                max_x = x;
            }
            if y > max_y {
                max_y = y;
            }
            i += dim;
        }
        inv_size = (max_x - min_x).max(max_y - min_y);
        inv_size = if inv_size != 0.0 { 32767.0 / inv_size } else { 0.0 };
    }
    e.earcut_linked(outer, min_x, min_y, inv_size, 0);
    e.triangles
}

fn point_in_triangle(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, px: f64, py: f64) -> bool {
    (cx - px) * (ay - py) >= (ax - px) * (cy - py)
        && (ax - px) * (by - py) >= (bx - px) * (ay - py)
        && (bx - px) * (cy - py) >= (cx - px) * (by - py)
}

fn point_in_triangle_except_first(ax: f64, ay: f64, bx: f64, by: f64, cx: f64, cy: f64, px: f64, py: f64) -> bool {
    !(ax == px && ay == py) && point_in_triangle(ax, ay, bx, by, cx, cy, px, py)
}

fn sign(num: f64) -> i64 {
    if num > 0.0 {
        1
    } else if num < 0.0 {
        -1
    } else {
        0
    }
}

fn signed_area(data: &Vec<f64>, start: i64, end: i64, dim: i64) -> f64 {
    let mut sum = 0.0;
    let mut j = end - dim;
    let mut i = start;
    while i < end {
        sum += (data[j as usize] - data[i as usize]) * (data[(i + 1) as usize] + data[(j + 1) as usize]);
        j = i;
        i += dim;
    }
    sum
}

fn z_order(x: f64, y: f64, min_x: f64, min_y: f64, inv_size: f64) -> i64 {
    let mut x = ((x - min_x) * inv_size) as i64;
    let mut y = ((y - min_y) * inv_size) as i64;
    x = (x | (x << 8)) & 0x00FF00FF;
    x = (x | (x << 4)) & 0x0F0F0F0F;
    x = (x | (x << 2)) & 0x33333333;
    x = (x | (x << 1)) & 0x55555555;
    y = (y | (y << 8)) & 0x00FF00FF;
    y = (y | (y << 4)) & 0x0F0F0F0F;
    y = (y | (y << 2)) & 0x33333333;
    y = (y | (y << 1)) & 0x55555555;
    x | (y << 1)
}

impl Earcut {
    fn nx(&self, p: i64) -> i64 {
        self.nodes[p as usize].next
    }

    fn pv(&self, p: i64) -> i64 {
        self.nodes[p as usize].prev
    }

    fn px(&self, p: i64) -> f64 {
        self.nodes[p as usize].x
    }

    fn py(&self, p: i64) -> f64 {
        self.nodes[p as usize].y
    }

    fn idx(&self, p: i64) -> i64 {
        self.nodes[p as usize].i
    }

    fn area(&self, p: i64, q: i64, r: i64) -> f64 {
        (self.py(q) - self.py(p)) * (self.px(r) - self.px(q)) - (self.px(q) - self.px(p)) * (self.py(r) - self.py(q))
    }

    fn equals(&self, a: i64, b: i64) -> bool {
        self.px(a) == self.px(b) && self.py(a) == self.py(b)
    }

    fn create_node(&mut self, i: i64, x: f64, y: f64) -> i64 {
        self.nodes.push(Node { i, x, y, prev: -1, next: -1, z: 0, prev_z: -1, next_z: -1, steiner: false });
        self.nodes.len() as i64 - 1
    }

    fn insert_node(&mut self, i: i64, x: f64, y: f64, last: i64) -> i64 {
        let p = self.create_node(i, x, y);
        if last < 0 {
            self.nodes[p as usize].prev = p;
            self.nodes[p as usize].next = p;
        } else {
            let ln = self.nx(last);
            self.nodes[p as usize].next = ln;
            self.nodes[p as usize].prev = last;
            self.nodes[ln as usize].prev = p;
            self.nodes[last as usize].next = p;
        }
        p
    }

    fn remove_node(&mut self, p: i64) {
        let n = self.nodes[p as usize];
        self.nodes[n.next as usize].prev = n.prev;
        self.nodes[n.prev as usize].next = n.next;
        if n.prev_z >= 0 {
            self.nodes[n.prev_z as usize].next_z = n.next_z;
        }
        if n.next_z >= 0 {
            self.nodes[n.next_z as usize].prev_z = n.prev_z;
        }
    }

    fn linked_list(&mut self, data: &Vec<f64>, start: i64, end: i64, dim: i64, clockwise: bool) -> i64 {
        let mut last: i64 = -1;
        if clockwise == (signed_area(data, start, end, dim) > 0.0) {
            let mut i = start;
            while i < end {
                last = self.insert_node(i / dim, data[i as usize], data[(i + 1) as usize], last);
                i += dim;
            }
        } else {
            let mut i = end - dim;
            while i >= start {
                last = self.insert_node(i / dim, data[i as usize], data[(i + 1) as usize], last);
                i -= dim;
            }
        }
        if last >= 0 && self.equals(last, self.nx(last)) {
            self.remove_node(last);
            last = self.nx(last);
        }
        last
    }

    fn filter_points(&mut self, start: i64, end_in: i64) -> i64 {
        if start < 0 {
            return start;
        }
        let mut end = if end_in < 0 { start } else { end_in };
        let mut p = start;
        loop {
            let mut again = false;
            if !self.nodes[p as usize].steiner && (self.equals(p, self.nx(p)) || self.area(self.pv(p), p, self.nx(p)) == 0.0) {
                self.remove_node(p);
                p = self.pv(p);
                end = p;
                if p == self.nx(p) {
                    break;
                }
                again = true;
            } else {
                p = self.nx(p);
            }
            if !(again || p != end) {
                break;
            }
        }
        end
    }

    fn earcut_linked(&mut self, ear_in: i64, min_x: f64, min_y: f64, inv_size: f64, pass: i64) {
        if ear_in < 0 {
            return;
        }
        let mut ear = ear_in;
        if pass == 0 && inv_size != 0.0 {
            self.index_curve(ear, min_x, min_y, inv_size);
        }
        let mut stop = ear;
        while self.pv(ear) != self.nx(ear) {
            let prev = self.pv(ear);
            let next = self.nx(ear);
            let is_ear = if inv_size != 0.0 { self.is_ear_hashed(ear, min_x, min_y, inv_size) } else { self.is_ear(ear) };
            if is_ear {
                let (pi, ei, ni) = (self.idx(prev), self.idx(ear), self.idx(next));
                self.triangles.push(pi);
                self.triangles.push(ei);
                self.triangles.push(ni);
                self.remove_node(ear);
                ear = self.nx(next);
                stop = self.nx(next);
                continue;
            }
            ear = next;
            if ear == stop {
                if pass == 0 {
                    let f = self.filter_points(ear, -1);
                    self.earcut_linked(f, min_x, min_y, inv_size, 1);
                } else if pass == 1 {
                    let f = self.filter_points(ear, -1);
                    let cured = self.cure_local_intersections(f);
                    self.earcut_linked(cured, min_x, min_y, inv_size, 2);
                } else if pass == 2 {
                    self.split_earcut(ear, min_x, min_y, inv_size);
                }
                break;
            }
        }
    }

    fn is_ear(&self, ear: i64) -> bool {
        let a = self.pv(ear);
        let b = ear;
        let c = self.nx(ear);
        if self.area(a, b, c) >= 0.0 {
            return false;
        }
        let (ax, bx, cx, ay, by, cy) = (self.px(a), self.px(b), self.px(c), self.py(a), self.py(b), self.py(c));
        let x0 = ax.min(bx).min(cx);
        let y0 = ay.min(by).min(cy);
        let x1 = ax.max(bx).max(cx);
        let y1 = ay.max(by).max(cy);
        let mut p = self.nx(c);
        while p != a {
            let (x, y) = (self.px(p), self.py(p));
            if x >= x0 && x <= x1 && y >= y0 && y <= y1
                && point_in_triangle_except_first(ax, ay, bx, by, cx, cy, x, y)
                && self.area(self.pv(p), p, self.nx(p)) >= 0.0
            {
                return false;
            }
            p = self.nx(p);
        }
        true
    }

    fn blocks_ear(&self, p: i64, a: i64, c: i64, x0: f64, y0: f64, x1: f64, y1: f64) -> bool {
        let (ax, ay, bx, by, cx, cy) = (self.px(a), self.py(a), self.px(self.nx(a)), self.py(self.nx(a)), self.px(c), self.py(c));
        let (x, y) = (self.px(p), self.py(p));
        x >= x0 && x <= x1 && y >= y0 && y <= y1 && p != a && p != c
            && point_in_triangle_except_first(ax, ay, bx, by, cx, cy, x, y)
            && self.area(self.pv(p), p, self.nx(p)) >= 0.0
    }

    fn is_ear_hashed(&self, ear: i64, min_x: f64, min_y: f64, inv_size: f64) -> bool {
        let a = self.pv(ear);
        let b = ear;
        let c = self.nx(ear);
        if self.area(a, b, c) >= 0.0 {
            return false;
        }
        let (ax, bx, cx, ay, by, cy) = (self.px(a), self.px(b), self.px(c), self.py(a), self.py(b), self.py(c));
        let x0 = ax.min(bx).min(cx);
        let y0 = ay.min(by).min(cy);
        let x1 = ax.max(bx).max(cx);
        let y1 = ay.max(by).max(cy);
        let min_z = z_order(x0, y0, min_x, min_y, inv_size);
        let max_z = z_order(x1, y1, min_x, min_y, inv_size);
        let mut p = self.nodes[ear as usize].prev_z;
        let mut n = self.nodes[ear as usize].next_z;
        while p >= 0 && self.nodes[p as usize].z >= min_z && n >= 0 && self.nodes[n as usize].z <= max_z {
            if self.blocks_ear(p, a, c, x0, y0, x1, y1) {
                return false;
            }
            p = self.nodes[p as usize].prev_z;
            if self.blocks_ear(n, a, c, x0, y0, x1, y1) {
                return false;
            }
            n = self.nodes[n as usize].next_z;
        }
        while p >= 0 && self.nodes[p as usize].z >= min_z {
            if self.blocks_ear(p, a, c, x0, y0, x1, y1) {
                return false;
            }
            p = self.nodes[p as usize].prev_z;
        }
        while n >= 0 && self.nodes[n as usize].z <= max_z {
            if self.blocks_ear(n, a, c, x0, y0, x1, y1) {
                return false;
            }
            n = self.nodes[n as usize].next_z;
        }
        true
    }

    fn cure_local_intersections(&mut self, start_in: i64) -> i64 {
        let mut start = start_in;
        let mut p = start;
        loop {
            let a = self.pv(p);
            let b = self.nx(self.nx(p));
            if !self.equals(a, b) && self.intersects(a, p, self.nx(p), b) && self.locally_inside(a, b) && self.locally_inside(b, a) {
                let (ai, pi, bi) = (self.idx(a), self.idx(p), self.idx(b));
                self.triangles.push(ai);
                self.triangles.push(pi);
                self.triangles.push(bi);
                self.remove_node(p);
                let pn = self.nx(p);
                self.remove_node(pn);
                p = b;
                start = b;
            }
            p = self.nx(p);
            if p == start {
                break;
            }
        }
        self.filter_points(p, -1)
    }

    fn split_earcut(&mut self, start: i64, min_x: f64, min_y: f64, inv_size: f64) {
        let mut a = start;
        loop {
            let mut b = self.nx(self.nx(a));
            while b != self.pv(a) {
                if self.idx(a) != self.idx(b) && self.is_valid_diagonal(a, b) {
                    let mut c = self.split_polygon(a, b);
                    let an = self.nx(a);
                    a = self.filter_points(a, an);
                    let cn = self.nx(c);
                    c = self.filter_points(c, cn);
                    self.earcut_linked(a, min_x, min_y, inv_size, 0);
                    self.earcut_linked(c, min_x, min_y, inv_size, 0);
                    return;
                }
                b = self.nx(b);
            }
            a = self.nx(a);
            if a == start {
                break;
            }
        }
    }

    fn eliminate_holes(&mut self, data: &Vec<f64>, hole_indices: &Vec<i64>, outer_in: i64, dim: i64) -> i64 {
        let mut outer = outer_in;
        let mut queue: Vec<i64> = Vec::new();
        let len = hole_indices.len();
        for i in 0..len {
            let start = hole_indices[i] * dim;
            let end = if i < len - 1 { hole_indices[i + 1] * dim } else { data.len() as i64 };
            let list = self.linked_list(data, start, end, dim, false);
            if list < 0 {
                continue;
            }
            if list == self.nx(list) {
                self.nodes[list as usize].steiner = true;
            }
            queue.push(self.get_leftmost(list));
        }
        // stable insertion sort, as Array.prototype.sort is
        for i in 1..queue.len() {
            let mut j = i;
            while j > 0 && self.compare_xy_slope(queue[j - 1], queue[j]) > 0.0 {
                queue.swap(j - 1, j);
                j -= 1;
            }
        }
        for q in queue.iter() {
            outer = self.eliminate_hole(*q, outer);
        }
        outer
    }

    fn compare_xy_slope(&self, a: i64, b: i64) -> f64 {
        let mut result = self.px(a) - self.px(b);
        if result == 0.0 {
            result = self.py(a) - self.py(b);
            if result == 0.0 {
                let an = self.nx(a);
                let bn = self.nx(b);
                let a_slope = (self.py(an) - self.py(a)) / (self.px(an) - self.px(a));
                let b_slope = (self.py(bn) - self.py(b)) / (self.px(bn) - self.px(b));
                result = a_slope - b_slope;
            }
        }
        result
    }

    fn eliminate_hole(&mut self, hole: i64, outer: i64) -> i64 {
        let bridge = self.find_hole_bridge(hole, outer);
        if bridge < 0 {
            return outer;
        }
        let bridge_reverse = self.split_polygon(bridge, hole);
        let brn = self.nx(bridge_reverse);
        self.filter_points(bridge_reverse, brn);
        let bn = self.nx(bridge);
        self.filter_points(bridge, bn)
    }

    fn find_hole_bridge(&self, hole: i64, outer: i64) -> i64 {
        let mut p = outer;
        let hx = self.px(hole);
        let hy = self.py(hole);
        let mut qx = -(1.0 / 0.0);
        let mut m: i64 = -1;
        if self.equals(hole, p) {
            return p;
        }
        loop {
            let pn = self.nx(p);
            if self.equals(hole, pn) {
                return pn;
            } else if hy <= self.py(p) && hy >= self.py(pn) && self.py(pn) != self.py(p) {
                let x = self.px(p) + (hy - self.py(p)) * (self.px(pn) - self.px(p)) / (self.py(pn) - self.py(p));
                if x <= hx && x > qx {
                    qx = x;
                    m = if self.px(p) < self.px(pn) { p } else { pn };
                    if x == hx {
                        return m;
                    }
                }
            }
            p = pn;
            if p == outer {
                break;
            }
        }
        if m < 0 {
            return -1;
        }
        let stop = m;
        let mx = self.px(m);
        let my = self.py(m);
        let mut tan_min = 1.0 / 0.0;
        p = m;
        loop {
            let (x, y) = (self.px(p), self.py(p));
            let ax = if hy < my { hx } else { qx };
            let cx = if hy < my { qx } else { hx };
            if hx >= x && x >= mx && hx != x && point_in_triangle(ax, hy, mx, my, cx, hy, x, y) {
                let tan = (hy - y).abs() / (hx - x);
                if self.locally_inside(p, hole)
                    && (tan < tan_min || (tan == tan_min && (x > self.px(m) || (x == self.px(m) && self.sector_contains_sector(m, p)))))
                {
                    m = p;
                    tan_min = tan;
                }
            }
            p = self.nx(p);
            if p == stop {
                break;
            }
        }
        m
    }

    fn sector_contains_sector(&self, m: i64, p: i64) -> bool {
        self.area(self.pv(m), m, self.pv(p)) < 0.0 && self.area(self.nx(p), m, self.nx(m)) < 0.0
    }

    fn index_curve(&mut self, start: i64, min_x: f64, min_y: f64, inv_size: f64) {
        let mut p = start;
        loop {
            if self.nodes[p as usize].z == 0 {
                let (x, y) = (self.px(p), self.py(p));
                self.nodes[p as usize].z = z_order(x, y, min_x, min_y, inv_size);
            }
            let (pp, pn) = (self.pv(p), self.nx(p));
            self.nodes[p as usize].prev_z = pp;
            self.nodes[p as usize].next_z = pn;
            p = pn;
            if p == start {
                break;
            }
        }
        let pz = self.nodes[p as usize].prev_z;
        self.nodes[pz as usize].next_z = -1;
        self.nodes[p as usize].prev_z = -1;
        self.sort_linked(p);
    }

    fn sort_linked(&mut self, list_in: i64) -> i64 {
        let mut list = list_in;
        let mut in_size: i64 = 1;
        loop {
            let mut p = list;
            list = -1;
            let mut tail: i64 = -1;
            let mut num_merges = 0;
            while p >= 0 {
                num_merges += 1;
                let mut q = p;
                let mut p_size = 0;
                for _ in 0..in_size {
                    p_size += 1;
                    q = self.nodes[q as usize].next_z;
                    if q < 0 {
                        break;
                    }
                }
                let mut q_size = in_size;
                while p_size > 0 || (q_size > 0 && q >= 0) {
                    let e: i64;
                    if p_size != 0 && (q_size == 0 || q < 0 || self.nodes[p as usize].z <= self.nodes[q as usize].z) {
                        e = p;
                        p = self.nodes[p as usize].next_z;
                        p_size -= 1;
                    } else {
                        e = q;
                        q = self.nodes[q as usize].next_z;
                        q_size -= 1;
                    }
                    if tail >= 0 {
                        self.nodes[tail as usize].next_z = e;
                    } else {
                        list = e;
                    }
                    self.nodes[e as usize].prev_z = tail;
                    tail = e;
                }
                p = q;
            }
            self.nodes[tail as usize].next_z = -1;
            in_size *= 2;
            if num_merges <= 1 {
                break;
            }
        }
        list
    }

    fn get_leftmost(&self, start: i64) -> i64 {
        let mut p = start;
        let mut leftmost = start;
        loop {
            if self.px(p) < self.px(leftmost) || (self.px(p) == self.px(leftmost) && self.py(p) < self.py(leftmost)) {
                leftmost = p;
            }
            p = self.nx(p);
            if p == start {
                break;
            }
        }
        leftmost
    }

    fn is_valid_diagonal(&self, a: i64, b: i64) -> bool {
        let (an, ap, bn, bp) = (self.nx(a), self.pv(a), self.nx(b), self.pv(b));
        self.idx(an) != self.idx(b)
            && self.idx(ap) != self.idx(b)
            && !self.intersects_polygon(a, b)
            && ((self.locally_inside(a, b) && self.locally_inside(b, a) && self.middle_inside(a, b)
                && (self.area(ap, a, bp) != 0.0 || self.area(a, bp, b) != 0.0))
                || (self.equals(a, b) && self.area(ap, a, an) > 0.0 && self.area(bp, b, bn) > 0.0))
    }

    fn on_segment(&self, p: i64, q: i64, r: i64) -> bool {
        self.px(q) <= self.px(p).max(self.px(r))
            && self.px(q) >= self.px(p).min(self.px(r))
            && self.py(q) <= self.py(p).max(self.py(r))
            && self.py(q) >= self.py(p).min(self.py(r))
    }

    fn intersects(&self, p1: i64, q1: i64, p2: i64, q2: i64) -> bool {
        let o1 = sign(self.area(p1, q1, p2));
        let o2 = sign(self.area(p1, q1, q2));
        let o3 = sign(self.area(p2, q2, p1));
        let o4 = sign(self.area(p2, q2, q1));
        if o1 != o2 && o3 != o4 {
            return true;
        }
        if o1 == 0 && self.on_segment(p1, p2, q1) {
            return true;
        }
        if o2 == 0 && self.on_segment(p1, q2, q1) {
            return true;
        }
        if o3 == 0 && self.on_segment(p2, p1, q2) {
            return true;
        }
        if o4 == 0 && self.on_segment(p2, q1, q2) {
            return true;
        }
        false
    }

    fn intersects_polygon(&self, a: i64, b: i64) -> bool {
        let mut p = a;
        let (ai, bi) = (self.idx(a), self.idx(b));
        loop {
            let pn = self.nx(p);
            if self.idx(p) != ai && self.idx(pn) != ai && self.idx(p) != bi && self.idx(pn) != bi && self.intersects(p, pn, a, b) {
                return true;
            }
            p = pn;
            if p == a {
                break;
            }
        }
        false
    }

    fn locally_inside(&self, a: i64, b: i64) -> bool {
        let (ap, an) = (self.pv(a), self.nx(a));
        if self.area(ap, a, an) < 0.0 {
            self.area(a, b, an) >= 0.0 && self.area(a, ap, b) >= 0.0
        } else {
            self.area(a, b, ap) < 0.0 || self.area(a, an, b) < 0.0
        }
    }

    fn middle_inside(&self, a: i64, b: i64) -> bool {
        let mut p = a;
        let mut inside = false;
        let px = (self.px(a) + self.px(b)) / 2.0;
        let py = (self.py(a) + self.py(b)) / 2.0;
        loop {
            let pn = self.nx(p);
            let (x, y, nx, ny) = (self.px(p), self.py(p), self.px(pn), self.py(pn));
            if ((y > py) != (ny > py)) && ny != y && (px < (nx - x) * (py - y) / (ny - y) + x) {
                inside = !inside;
            }
            p = pn;
            if p == a {
                break;
            }
        }
        inside
    }

    fn split_polygon(&mut self, a: i64, b: i64) -> i64 {
        let (ai, ax, ay) = (self.idx(a), self.px(a), self.py(a));
        let (bi, bx, by) = (self.idx(b), self.px(b), self.py(b));
        let a2 = self.create_node(ai, ax, ay);
        let b2 = self.create_node(bi, bx, by);
        let an = self.nx(a);
        let bp = self.pv(b);
        self.nodes[a as usize].next = b;
        self.nodes[b as usize].prev = a;
        self.nodes[a2 as usize].next = an;
        self.nodes[an as usize].prev = a2;
        self.nodes[b2 as usize].next = a2;
        self.nodes[a2 as usize].prev = b2;
        self.nodes[bp as usize].next = b2;
        self.nodes[b2 as usize].prev = bp;
        b2
    }
}
