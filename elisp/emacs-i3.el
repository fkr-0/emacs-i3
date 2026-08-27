;;; emacs-i3.el --- Route i3-style window commands through Emacs -*- lexical-binding: t; -*-

;;; Commentary:
;; Each command returns non-nil only when Emacs handled it.  A caller can then
;; fall back to i3 whenever no suitable Emacs window or operation exists.

;;; Code:

(require 'cl-lib)
(require 'seq)
(require 'windmove)

(defgroup emacs-i3 nil
  "Coordinate Emacs windows with an i3 command bridge."
  :group 'windows)

(defcustom emacs-i3-skip-minibuffer t
  "Do not focus or swap with minibuffer windows."
  :type 'boolean
  :group 'emacs-i3)

(defvaralias 'my/emacs-i3-skip-minibuffer 'emacs-i3-skip-minibuffer)

(defun emacs-i3--read-direction ()
  "Read and return a windmove direction symbol."
  (intern (completing-read "Direction: " '(up down left right) nil t)))

(defun emacs-i3--target-window (direction)
  "Return the live neighboring window in DIRECTION, or nil."
  (when (memq direction '(up down left right))
    (let ((window (windmove-find-other-window direction)))
      (when (and (window-live-p window)
                 (or (not emacs-i3-skip-minibuffer)
                     (not (window-minibuffer-p window))))
        window))))

(defun my/emacs-i3-focus (direction)
  "Focus inside Emacs in DIRECTION, or return nil for i3 fallback."
  (interactive (list (emacs-i3--read-direction)))
  (let ((origin (selected-window)))
    (condition-case nil
        (progn
          ;; Use Windmove's public selector so side/atomic windows retain its
          ;; normal semantics instead of selecting a guessed target directly.
          (windmove-do-window-select direction)
          (if (and emacs-i3-skip-minibuffer
                   (window-minibuffer-p (selected-window)))
              (progn
                (select-window origin)
                nil)
            t))
      (error
       (when (window-live-p origin)
         (select-window origin))
       nil))))

(defun my/emacs-i3-direction-exists-p (axis)
  "Return non-nil when AXIS has at least one neighboring Emacs window."
  (cl-some #'emacs-i3--target-window
           (pcase axis
             ('width '(left right))
             ('height '(up down))
             (_ nil))))

(defun my/emacs-i3-move (direction)
  "Swap the selected window with its neighbor in DIRECTION."
  (interactive (list (emacs-i3--read-direction)))
  (when-let ((window (emacs-i3--target-window direction)))
    (window-swap-states (selected-window) window)
    t))

(defun emacs-i3--resize-amount (arguments)
  "Return the first positive numeric resize amount in ARGUMENTS."
  (or (seq-some
       (lambda (argument)
         (when (and (stringp argument)
                    (string-match-p "\\`[0-9]+\\'" argument))
           (max 1 (string-to-number argument))))
       arguments)
      1))

(defun my/emacs-i3-resize (direction axis arguments)
  "Resize the selected window along AXIS in DIRECTION.

ARGUMENTS may contain an i3 pixel or percentage amount; Emacs interprets the
first positive integer as columns or lines.  Return nil when i3 should handle
the command instead."
  (when (and (not (one-window-p))
             (my/emacs-i3-direction-exists-p axis))
    (let ((amount (emacs-i3--resize-amount arguments)))
      (condition-case nil
          (progn
            (pcase (list direction axis)
              (`(shrink width)  (shrink-window-horizontally amount))
              (`(shrink height) (shrink-window amount))
              (`(grow width)    (enlarge-window-horizontally amount))
              (`(grow height)   (enlarge-window amount))
              (_ (user-error "Unsupported resize command")))
            t)
        (error nil)))))

(defun my/emacs-i3-split (direction)
  "Split in DIRECTION (`h' or `v') and select the new window."
  (let ((window
         (pcase direction
           ((or 'h "h") (split-window-right))
           ((or 'v "v") (split-window-below))
           (_ nil))))
    (when (window-live-p window)
      (select-window window)
      t)))

(defun my/emacs-i3-kill ()
  "Delete the selected Emacs window, or return nil for i3 fallback."
  (unless (one-window-p)
    (delete-window)
    t))

(defun emacs-i3--transpose-layout ()
  "Transpose the current frame layout when the command is available."
  (when (fboundp 'transpose-frame)
    (transpose-frame)
    t))

(defun my/emacs-i3-command (command)
  "Try to execute i3-style COMMAND inside Emacs.

Return non-nil when Emacs handled the command, otherwise nil so the external
window manager can process it."
  (pcase (split-string command nil t)
    (`("focus" ,direction)
     (my/emacs-i3-focus (intern direction)))
    (`("move" ,direction)
     (my/emacs-i3-move (intern direction)))
    (`("resize" ,direction ,axis . ,arguments)
     (my/emacs-i3-resize (intern direction) (intern axis) arguments))
    (`("layout" "toggle" "split")
     (emacs-i3--transpose-layout))
    (`("split" ,direction)
     (my/emacs-i3-split direction))
    (`("kill")
     (my/emacs-i3-kill))
    (_ nil)))

(provide 'emacs-i3)
;;; emacs-i3.el ends here
