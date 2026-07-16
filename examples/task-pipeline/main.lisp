(import tasks "tasks")

(type task-outcome
  (accepted int)
  (rejected str))

(def evaluate
  (fn (title weights effort urgency)
    (let (normalized (tasks.normalize-title title)
          score (tasks.weighted-total weights effort urgency))
      (if (> (str.len normalized) 0)
        (accepted score)
        (rejected "title required")))))

(export main
  (fn ()
    (case (evaluate " ship native subset " (list 10 20) 5 7)
      ((accepted score) score)
      ((rejected _) 0))))
