(import format "format")

(type report-result
  (ready int)
  (failed str))

(def build-report
  (fn (heading values)
    (if (list.every (fn (value) (> value 0)) values)
      (ready (format.heading-score heading values))
      (failed "non-positive metric"))))

(export main
  (fn ()
    (case (build-report "ok" (list 10 20 10))
      ((ready score) score)
      ((failed _) 0))))
