class Block
  attr_accessor :piece, :player
  attr_reader :x, :y

  class << self
    def parse(x, y, value)

      player = value[0] == '-' ? nil : value[0].to_i
      piece = value[1, 4] == '----' ? nil : Piece.parse(value[1, 4])

      new x: x, y: y, player: player, piece: piece
    end
  end

  def initialize(x:, y:, player: nil, piece: nil)
    @player = player
    @piece = piece
  end

  def to_s
    "#{@player ? @player.to_s : '-'}#{@piece ? @piece.to_s : '----'}"
  end
end
