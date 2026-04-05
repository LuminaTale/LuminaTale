lumina.tween = require "system.core.tween"
lumina.log = require "system.core.log"
lumina.log.info("⚡ Loading Engine Extensions...")

function lumina_update(dt)
    lumina.tween.update(dt)
    f._display = lumina.typewriter.get()
end

require "system.layouts"
require "system.transitions"
require "system.effects"

function lumina_on_dialogue(speaker, text)
    f._speaker = speaker or ""
    lumina.typewriter.set(text)
    lumina.ui.show_screen("dialogue")
end

function lumina_on_narration(text)
    f._speaker = ""
    lumina.typewriter.set(text)
    lumina.ui.show_screen("dialogue")
end

function lumina_on_choice(title, options)
    f._choice_title = title or "选择"
    f._choice_0 = options[1] or ""
    f._choice_1 = options[2] or ""
    lumina.ui.show_screen("choice_menu")
end

lumina.log.info("Systems Ready.")
