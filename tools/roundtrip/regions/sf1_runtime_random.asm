; Retail $02:FC58-$02:FC6F. The long-call entry and four-byte runtime RNG.
runtime_random_long_entry:
  JSR runtime_random
  RTL

runtime_random:
  LDA $EF
  CLC
  SBC $F0
  STA $F0
  SBC $F1
  STA $F1
  SBC $F2
  STA $F2
  SBC $EF
  STA $EF
  RTS
