; A local resource belongs to this mounted component, not to the app root.
; The playground provider executes storage.write@1, then completion reaches update.

(type Action (Saved obj) (SaveFailed obj))

(def init
  (obj "last-save" "Not saved yet"))

(defn update (state action)
  (case action
    ((Saved result)
      (obj.set state "last-save" (str.cat "Saved " (. result "key"))))
    ((SaveFailed error)
      (obj.set state "last-save" (str.cat "Save failed: " (. error "code"))))))

(component save-card (key value)
  (ui.local false (fn (saving set-saving)
    (section
      (class "rounded-xl" "border" "border-slate-700" "bg-slate-900/70" "p-5" "shadow-lg")
      (h2 (class "text-lg" "font-semibold" "text-white") (text key))
      (p (class "mt-1" "text-sm" "text-slate-400")
        (text "The command is owned by this card's local scope."))
      (button
        (class "mt-4" "rounded-lg" "bg-cyan-400" "px-4" "py-2" "font-semibold" "text-slate-950")
        (on click
          (emit
            (ui.local.result true
              (list
                (ui.command "save" "storage.write" 1
                  (obj "key" key "value" value)
                  false
                  (ui.action-result "Saved" (list))
                  (ui.action-error "SaveFailed" (list))))
              (list))))
        (text (if saving "Saved locally" "Save this card")))))))

(component app (state)
  (main
    (class "mx-auto" "max-w-3xl" "space-y-5" "p-8" "text-slate-100")
    (div
      (p (class "text-sm" "font-medium" "text-cyan-300") (text "LOCAL EFFECT OWNERSHIP"))
      (h1 (class "mt-1" "text-3xl" "font-bold") (text "Two cards, one resource id"))
      (p (class "mt-2" "text-slate-400") (text (. state "last-save"))))
    (div
      (class "grid" "gap-4" "md:grid-cols-2")
      (save-card "draft:one" (obj "title" "First draft"))
      (save-card "draft:two" (obj "title" "Second draft")))))

(ui.app init update app)
