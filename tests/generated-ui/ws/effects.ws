type Action
  (Save)
  Saved int
  SaveFailed obj
  (StartClock)
  Tick int

def init 0

defn update (state action)
  case action
    (Save)
      ui.result state
        list
          ui.command "save:1" "storage.write" 1
            obj "key" "draft"
            true
            ui.action-result "Saved"
              (list)
            ui.action-error "SaveFailed"
              (list)
        (list)
    (Saved revision)
      ui.result revision
        (list)
        (list)
    (SaveFailed _)
      ui.result -1
        (list)
        (list)
    (StartClock)
      ui.result state
        (list)
        list
          ui.subscription "clock" "timer.tick" 1
            obj "every-ms" 1000
            false
            ui.action-result "Tick"
              (list)
            ui.action-error "SaveFailed"
              (list)
    (Tick tick)
      ui.result tick
        (list)
        (list)

component app
  (state)
  main
    attr "aria-label" "Effect test"
    text
      str.from state

ui.app init update app

ui.test "command completion reaches the reducer"
  supports "storage.write" 1
  dispatch Save
  assert
    =
      (list)
      (ui.test.subscriptions)
  deliver command "save:1" 42
  assert
    = 42
      (ui.test.state)
  assert
    =
      (list)
      (ui.test.commands)

ui.test "command error reaches the reducer"
  supports "storage.write" 1
  dispatch Save
  deliver-error command "save:1" "permission-denied" "readonly"
  assert
    = -1
      (ui.test.state)

ui.test "subscription completion reaches the reducer"
  supports "timer.tick" 1
  dispatch StartClock
  deliver subscription "clock" 7
  assert
    = 7
      (ui.test.state)
