;; Scope analysis for Clojure (and let-go / Clojure-flavored Lisps).
;;
;; tree-sitter-clojure uniformly represents code as `list_lit` nodes
;; containing `sym_lit` / `vec_lit` / `map_lit` children. To recognize
;; defining forms (defn, def, let, etc.) we match by the head symbol.
;;
;; Captures used by bonsai (see crates/bonsai-core/src/scope.rs):
;;   @local.scope        — a region introducing a new lexical scope
;;   @local.definition   — a binding within the enclosing scope
;;   @local.reference    — a use of a name that should resolve to a definition
;;
;; A node tagged with all three (e.g. a `let` form) creates a scope and
;; the identifiers within `vec_lit` bindings are definitions visible to
;; subsequent siblings.

;; ============================================================================
;; Scopes
;; ============================================================================
;;
;; Every form that introduces a lexical environment becomes a scope.
;; Function bodies, let/letfn/loop bindings, and the source file itself.

(source) @local.scope

;; Function-defining forms — the whole list is a scope (body sees args).
((list_lit
   .
   (sym_lit (sym_name) @_head)
   .
   _*)
 (#match? @_head "^(defn|defn-|defmacro|fn)$"))
 @local.scope

;; let / let-like binding forms.
((list_lit
   .
   (sym_lit (sym_name) @_head)
   .
   _*)
 (#match? @_head "^(let|let\\*|letfn|loop|when-let|if-let|when-some|if-some|binding|with-open|with-local-vars|for|doseq)$"))
 @local.scope

;; ============================================================================
;; Definitions
;; ============================================================================
;;
;; Top-level: (def name expr), (defn name [params] body), etc.
;; The defined name is the second symbol in the list.

((list_lit
   .
   (sym_lit (sym_name) @_head)
   .
   (sym_lit (sym_name) @local.definition))
 (#match? @_head "^(def|defn|defn-|defmacro|defmulti|defmethod|defrecord|defprotocol|deftype|deftrait|defitem|defcreature|defprop)$"))

;; Function parameter vectors: (fn [a b c] ...) / (defn name [a b c] ...)
;; tree-sitter-clojure parses [a b c] as vec_lit containing sym_lit children.
;; Each symbol in the parameter vector is a definition in the enclosing scope.

((list_lit
   .
   (sym_lit (sym_name) @_head)
   .
   (sym_lit) ;; defn name (skip — already a definition above)
   .
   (vec_lit (sym_lit (sym_name) @local.definition)))
 (#match? @_head "^(defn|defn-|defmacro)$"))

;; (fn [a b c] body) — anonymous, no name to skip
((list_lit
   .
   (sym_lit (sym_name) @_head)
   .
   (vec_lit (sym_lit (sym_name) @local.definition)))
 (#match? @_head "^fn$"))

;; let-bindings: (let [a 1 b 2 c 3] body)
;; Each odd-indexed child of the bindings vector is a name. tree-sitter
;; doesn't natively expose "odd-indexed" but a vec_lit of sym/expr pairs
;; can be matched by recognizing the binding-vec as a sibling of the body.
;; We over-capture: every sym_lit in the bindings vector is treated as a
;; potential definition. (Bonsai handles false positives gracefully: an
;; unused name is harmless; an erroneously-defined name in a position
;; where it's actually an expression just adds noise to scope analysis.)

((list_lit
   .
   (sym_lit (sym_name) @_head)
   .
   (vec_lit (sym_lit (sym_name) @local.definition)))
 (#match? @_head "^(let|let\\*|loop|when-let|if-let|when-some|if-some|binding|with-open|with-local-vars|for|doseq)$"))

;; ============================================================================
;; References
;; ============================================================================
;;
;; Any symbol use that isn't in a defining position. We capture broadly
;; and let scope analysis disambiguate.

(sym_lit (sym_name) @local.reference)
