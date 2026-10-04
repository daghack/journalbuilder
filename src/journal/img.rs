    pub fn pixmap_from_svg_tree(svg_tree: &usvg::Tree, scale_width: f32, scale_height: f32) -> tiny_skia::Pixmap {
        let bounds = svg_tree.size();
        let width = (bounds.width() * scale_width).floor() as u32;
        let height = (bounds.height() * scale_height).floor() as u32;
        let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("Failed to create Pixmap");
        let transform = resvg::tiny_skia::Transform::identity();
        let transform = transform.pre_scale(scale_width, scale_height);
        resvg::render(&svg_tree, transform, &mut pixmap.as_mut());
        pixmap
    }
