; A category filter over immutable transaction data.
(type Action (Filter str))

(def init
  (obj
    "filter" "all"
    "transactions" (list
      (obj "id" 1 "title" "Northstar Coffee" "category" "food" "amount" 12 "icon" "☕")
      (obj "id" 2 "title" "Rail pass" "category" "travel" "amount" 48 "icon" "↔")
      (obj "id" 3 "title" "Design conference" "category" "work" "amount" 96 "icon" "✦")
      (obj "id" 4 "title" "Weekend market" "category" "food" "amount" 31 "icon" "♧"))))

(defn update (state action)
  (case action
    ((Filter value) (obj.set state "filter" value))))

(defn visible-transactions (state)
  (list.filter
    (fn (transaction)
      (or (= (. state "filter") "all") (= (. state "filter") (. transaction "category"))))
    (. state "transactions")))

(defn total (transactions)
  (list.fold (fn (sum transaction) (+ sum (. transaction "amount"))) 0 transactions))

(component category (current value label)
  (button
    (attr type "button")
    (class "rounded-xl" "border" "px-3" "py-2" "text-xs" "font-bold" "transition")
    (class-if "border-emerald-600" (= current value))
    (class-if "bg-emerald-600" (= current value))
    (class-if "text-white" (= current value))
    (class-if "border-slate-200" (not (= current value)))
    (class-if "bg-white" (not (= current value)))
    (class-if "text-slate-600" (not (= current value)))
    (on click (emit (Filter value)))
    (text label)))

(component transaction-row (transaction)
  (li
    (key (. transaction "id"))
    (class "flex" "items-center" "gap-3" "py-4")
    (span (class "grid" "size-10" "place-items-center" "rounded-xl" "bg-emerald-50" "text-lg" "text-emerald-700") (text (. transaction "icon")))
    (div
      (class "min-w-0" "flex-1")
      (p (class "truncate" "text-sm" "font-bold" "text-slate-900") (text (. transaction "title")))
      (p (class "mt-1" "text-xs" "font-semibold" "uppercase" "tracking-wider" "text-slate-400") (text (. transaction "category"))))
    (strong (class "text-sm" "font-black" "text-slate-900") (text (str "$" ,(str.from (. transaction "amount")))))))

(component app (state)
  (main
    (class "mx-auto" "max-w-xl" "p-6" "font-sans")
    (section
      (class "overflow-hidden" "rounded-3xl" "border" "border-slate-200" "bg-white" "shadow-xl")
      (div
        (class "bg-emerald-600" "p-6" "text-white")
        (p (class "text-xs" "font-bold" "uppercase" "tracking-[0.2em]" "text-emerald-100") (text "July overview"))
        (div
          (class "mt-3" "flex" "items-end" "justify-between")
          (div
            (h1 (class "text-3xl" "font-black") (text "Personal spend"))
            (p (class "mt-1" "text-sm" "text-emerald-100") (text "A small, portable finance view.")))
          (strong (class "text-3xl" "font-black") (text (str "$" ,(str.from (total (visible-transactions state))))))))
      (div
        (class "p-5")
        (div
          (class "flex" "flex-wrap" "gap-2")
          (category (. state "filter") "all" "Everything")
          (category (. state "filter") "food" "Food")
          (category (. state "filter") "travel" "Travel")
          (category (. state "filter") "work" "Work"))
        (ul
          (class "mt-3" "divide-y" "divide-slate-100")
          (for transaction (visible-transactions state)
            (transaction-row transaction)))))))

(ui.app init update app)
