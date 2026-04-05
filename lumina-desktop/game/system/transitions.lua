local reg = lumina.register_transition

reg("slide_in_right", {
    duration = 0.6,
    easing = "ease_out",
    props = {
        x = { from = 1920 + 200, to = 1440 }, -- 假设 1440 是 right_spot 的 x 坐标
        alpha = { from = 0.0, to = 1.0 }
    }
})

reg("dissolve", {
    duration = 0.3,
    easing = "ease_in_out",
    props = {
        alpha = { from = 0.0, to = 1.0 }
    }
})

-- "fade" 用于场景切换的淡入淡出交叉溶解
reg("fade", {
    duration = 0.5,
    easing = "ease_in_out",
    props = {}  -- 场景切换由内部 trans_progress 驱动，无需额外属性
})

reg("fade_in", {
    duration = 0.5,
    easing = "linear",
    props = {
        alpha = { from = 0.0, to = 1.0}
    }
})

reg("fade_out", {
    duration = 0.5,
    easing = "linear",
    props = {
        alpha = { from = 1.0, to = 0.0}
    }
})

reg("none", {
    duration = 0.0,
    easing = "linear",
    props = {}
})

reg("ease_move", {
    duration = 0.6,
    easing = "ease_in_out",
    props = {}
})

reg("circle_open", {
    duration = 1.5,
    easing = "ease_in_out",
    mask_img = "rules/circle.png",
    vague = 0.2,
})

reg("blinds_horiz", {
    duration = 1.0,
    easing = "linear", -- 百叶窗通常用线性比较自然
    mask_img = "rules/blinds_horizontal.png",
    vague = 0.05       -- 边缘稍微锐利一点
})

reg("blinds_vert", {
    duration = 1.0,
    easing = "linear",
    mask_img = "rules/blinds_vertical.png",
    vague = 0.05
})

lumina.log.info("Transitions loaded.")