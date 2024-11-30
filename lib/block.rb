class Block
  attr_accessor :piece, :player
  attr_reader :x, :y, :resource

  RESOURCES = [
    WOOD = :wood,
    STONE = :stone,
    IRON = :iron,
    GOLD = :gold
  ]

  class << self
    def parse(x, y, value)
      raise 'Invalid value size' unless value.size == 6

      resource =
        case value[0]
        when '-'
          nil
        when 'W'
          :wood
        when 'S'
          :stone
        when 'I'
          :iron
        when 'G'
          :gold
        else
          raise ArgumentError, "Invalid resource #{value[0]}"
        end

      player = value[1] == '-' ? nil : value[1].to_i
      piece = value[2, 5] == '----' ? nil : Piece.parse(value[2, 5])

      new x: x, y: y, player: player, piece: piece, resource: resource
    end
  end

  def initialize(x:, y:, player: nil, piece: nil, resource: nil)
    @player = player
    @piece = piece
    @resource =
      if resource && !RESOURCES.include?(resource)
        raise ArgumentError, "Invalid resource: #{resource}"
      else
        resource
      end
  end

  def to_s
    "#{@player ? @player.to_s : '-'}#{@piece ? @piece.to_s : '----'}"
  end
end
