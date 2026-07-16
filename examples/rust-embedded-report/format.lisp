(export heading-score
  (fn (heading values)
    (+ (str.len (str.upper heading))
       (list.fold (fn (total value) (+ total value)) 0 values))))
