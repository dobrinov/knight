class Rook < Piece
  TYPE = 'R'
  ATTACK = 1
  DEFENSE = 0
  HEALTH = 1

  MOVE_PATTERN = [
    [ 0,  1],
    [ 0, -1],
    [ 1,  0],
    [-1,  0],
  ]

  MOVE_PATTERN_REPETITION_LIMIT = nil
end