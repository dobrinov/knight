class Map
  class << self
    def parse(value)
      state = value.split("\n").map { _1.split('') }
      new(state)
    end
  end

  def initialize(state)
    @state = state
  end

  def to_board(players)
    blocks =
      @state.each_with_index.map do |row, x|
        row.each_with_index.map do |block, y|
          Block.new(x:, y:)
        end
      end

    Board.new blocks
  end
end
