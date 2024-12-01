class Map
  class << self
    def parse(value)
      state =
        value.split("\n").map do |row|
          blocks = row.split ''

          blocks.each_with_index do |block, x|
            raise ArgumentError, "Invalid block #{block}" unless ['X', '-', 'W', 'S', 'I', 'G'].include? block
          end

          blocks
        end

      new(state)
    end
  end

  def initialize(state)
    @state = state
  end

  def height
    @state.size
  end

  def width
    @state[0].size
  end

  def to_board(players_to_place)
    placed_players = 0

    blocks =
      @state.each_with_index.map do |row, x|
        row.each_with_index.map do |block, y|
          if block == 'X' && placed_players < players_to_place
            player = placed_players
            placed_players += 1
            piece = King.new

            Block.new x:, y:, player:, piece:
          elsif block == '-'
            Block.new x:, y:
          else
            Block.new x:, y:, resource: Resource.parse(block)
          end
        end
      end

    raise 'Not enough space on this board for all players' if placed_players < players_to_place

    Board.new blocks
  end
end
