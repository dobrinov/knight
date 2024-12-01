class Block
  attr_accessor :piece, :player
  attr_reader :x, :y, :resource

  class << self
    def parse(x, y, value)
      raise 'Invalid value size' unless value.size == 6

      resource = Resource.parse(value[0])
      player = value[1] == '-' ? nil : value[1].to_i
      piece = value[2, 5] == '----' ? nil : Piece.parse(value[2, 5])

      new x: x, y: y, player: player, piece: piece, resource: resource
    end
  end

  def initialize(x:, y:, player: nil, piece: nil, resource: nil)
    raise ArgumentError, "Invalid resource: #{resource}" unless Resource.valid?(resource)

    @player = player
    @piece = piece
    @resource = resource if resource
  end

  def to_s
    "#{@resource ? @resource : '-'}#{@player ? @player.to_s : '-'}#{@piece ? @piece.to_s : '----'}"
  end
end
