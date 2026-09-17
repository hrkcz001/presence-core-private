;;; Presence GNU Guix Package Manifest
(define-module (presence packages)
  #:use-module (guix packages)
  #:use-module (guix git-download)
  #:use-module (guix build-system cargo)
  #:use-module (guix build-system copy)
  #:use-module ((guix licenses) #:prefix license:)
  #:use-module (gnu packages pkg-config)
  #:use-module (gnu packages tls))

(define-public presence
  (package
    (name "presence")
    (version "0.3.0")
    (source (local-file "../.." #:recursive? #t))
    (build-system cargo-build-system)
    (native-inputs
     (list pkg-config))
    (inputs
     (list openssl))
    (synopsis "Autonomous conscious agent triad (Cortex, Stem, Cord)")
    (description
     "Presence is an autonomous Dasein cognitive agent system that executes
a hermeneutic observation-planning-verification-execution cycle with peripheral
sensory and effector organs.")
    (home-page "https://github.com/hrkcz001/presence")
    (license license:expat)))

(define-public presence-organ-vox
  (package
    (name "presence-organ-vox")
    (version "1.0.0")
    (source (local-file "../../organs/vox" #:recursive? #t))
    (build-system copy-build-system)
    (arguments
     '(#:install-plan
       '(("." "share/presence/organs/vox"))))
    (synopsis "Pure-Rust native voice organ for Presence")
    (description "Voice synthesis and audio capture organ for Presence.")
    (home-page "https://github.com/hrkcz001/presence")
    (license license:expat)))

(define-public presence-organ-git
  (package
    (name "presence-organ-git")
    (version "1.0.0")
    (source (local-file "../../organs/git" #:recursive? #t))
    (build-system copy-build-system)
    (arguments
     '(#:install-plan
       '(("." "share/presence/organs/git"))))
    (synopsis "Git interaction organ for Presence")
    (description "Git version control effector and safety organ.")
    (home-page "https://github.com/hrkcz001/presence")
    (license license:expat)))

(define-public presence-organ-io
  (package
    (name "presence-organ-io")
    (version "1.0.0")
    (source (local-file "../../organs/io" #:recursive? #t))
    (build-system copy-build-system)
    (arguments
     '(#:install-plan
       '(("." "share/presence/organs/io"))))
    (synopsis "Filesystem I/O organ for Presence")
    (description "Safe filesystem interaction organ for Presence.")
    (home-page "https://github.com/hrkcz001/presence")
    (license license:expat)))