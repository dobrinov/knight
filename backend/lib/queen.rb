class Queen < Piece
  TYPE = 'Q'
  ATTACK = 1
  DEFENSE = 0
  HEALTH = 1

  MOVE_PATTERN = [
    [ 0,  1],
    [ 0, -1],
    [ 1,  0],
    [ 1,  1],
    [ 1, -1],
    [-1,  0],
    [-1,  1],
    [-1, -1],
  ]

  MOVE_PATTERN_REPETITION_LIMIT = nil
end
