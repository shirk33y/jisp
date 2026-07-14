; Move cards across a small product board. The host only forwards events.
(type Action
  (Move int str)
  (Select str))

(def init
  (obj
    "selected" "all"
    "cards" (list
      (obj "id" 1 "title" "Outline the onboarding story" "stage" "ideas" "owner" "AR")
      (obj "id" 2 "title" "Prototype the empty state" "stage" "build" "owner" "ML")
      (obj "id" 3 "title" "Test the billing recovery flow" "stage" "build" "owner" "SK")
      (obj "id" 4 "title" "Publish the release notes" "stage" "done" "owner" "JO"))))

(defn next-stage (stage)
  (if (= stage "ideas")
    "build"
    (if (= stage "build") "done" "ideas")))

(defn move-card (card id stage)
  (if (= (. card "id") id)
    (obj.set card "stage" stage)
    card))

(defn update (state action)
  (case action
    ((Move id stage)
      (obj.set state "cards" (list.map (fn (card) (move-card card id stage)) (. state "cards"))))
    ((Select stage) (obj.set state "selected" stage))))

(defn stage-cards (state stage)
  (list.filter
    (fn (card)
      (and
        (= (. card "stage") stage)
        (or (= (. state "selected") "all") (= (. state "selected") stage))))
    (. state "cards")))

(component stage-tab (current stage label)
  (button
    (attr type "button")
    (class "rounded-lg" "px-3" "py-1.5" "text-sm" "font-semibold" "transition")
    (class-if "bg-white" (= current stage))
    (class-if "text-slate-900" (= current stage))
    (class-if "text-slate-300" (not (= current stage)))
    (on click (emit (Select stage)))
    (text label)))

(component task-card (card)
  (article
    (key (. card "id"))
    (class "rounded-xl" "border" "border-slate-200" "bg-white" "p-4" "shadow-sm")
    (p (class "text-sm" "font-semibold" "text-slate-900") (text (. card "title")))
    (div
      (class "mt-4" "flex" "items-center" "justify-between")
      (span (class "rounded-full" "bg-slate-100" "px-2" "py-1" "text-xs" "font-bold" "text-slate-600") (text (. card "owner")))
      (button
        (attr type "button")
        (class "rounded-lg" "bg-slate-900" "px-3" "py-1.5" "text-xs" "font-bold" "text-white" "hover:bg-indigo-600")
        (on click (emit (Move (. card "id") (next-stage (. card "stage")))))
        (text (if (= (. card "stage") "done") "Reopen" "Advance →"))))))

(component column (state stage title)
  (section
    (class "rounded-2xl" "border" "border-slate-200" "bg-slate-50" "p-3")
    (div
      (class "mb-3" "flex" "items-center" "justify-between")
      (h2 (class "text-sm" "font-black" "uppercase" "tracking-wider" "text-slate-700") (text title))
      (span
        (class "rounded-full" "px-2" "py-1" "text-xs" "font-bold" "text-white")
        (class-if "bg-amber-500" (= stage "ideas"))
        (class-if "bg-indigo-500" (= stage "build"))
        (class-if "bg-emerald-500" (= stage "done"))
        (text (str.from (list.len (stage-cards state stage))))))
    (div
      (class "space-y-3")
      (for card (stage-cards state stage)
        (task-card card)))))

(component app (state)
  (main
    (class "mx-auto" "max-w-6xl" "p-6" "font-sans")
    (section
      (class "overflow-hidden" "rounded-3xl" "bg-slate-950" "p-6" "shadow-2xl")
      (div
        (class "flex" "flex-col" "gap-4" "md:flex-row" "md:items-end" "md:justify-between")
        (div
          (p (class "text-xs" "font-bold" "uppercase" "tracking-[0.2em]" "text-indigo-300") (text "Pulse workspace"))
          (h1 (class "mt-2" "text-3xl" "font-black" "text-white") (text "Product launch board"))
          (p (class "mt-2" "max-w-xl" "text-sm" "text-slate-300") (text "Advance a card to see the immutable update flow in action.")))
        (div
          (class "flex" "rounded-xl" "bg-slate-800" "p-1")
          (stage-tab (. state "selected") "all" "All")
          (stage-tab (. state "selected") "ideas" "Ideas")
          (stage-tab (. state "selected") "build" "Build")
          (stage-tab (. state "selected") "done" "Done"))))
    (div
      (class "mt-6" "grid" "gap-4" "lg:grid-cols-3")
      (column state "ideas" "Ideas")
      (column state "build" "In build")
      (column state "done" "Done"))))

(ui.app init update app)
