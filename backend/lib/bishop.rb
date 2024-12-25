class Bishop < Piece
  TYPE = 'B'
  ATTACK = 1
  DEFENSE = 0
  HEALTH = 1

  MOVE_PATTERN = [
    [ 1,  1],
    [ 1, -1],
    [-1,  1],
    [-1, -1],
  ]

  MOVE_PATTERN_REPETITION_LIMIT = nil
end
