class Board
  class << self
    def parse(value)
      state =
        value.
          split("\n").map do |row, x|
            row.split(" ").map do |block, y|
              Block.parse(x, y, block)
            end
          end

      new(state)
    end
  end

  def initialize(state)
    @state = state
  end

  def move_piece(from, to, piece, player)
    raise "Invalid move" unless within?(*to)
    remove_piece *from
    place_piece to[0], to[1], piece, player
  end

  def place_piece(x, y, piece, player)
    raise "Invalid position" unless within?(x, y)
    raise "[#{x}, #{y}] is occupied by another piece" unless @state[x][y].piece.nil?
    @state[x][y].player = player
    @state[x][y].piece = piece
  end

  def remove_piece(x, y)
    raise "Invalid position" unless within?(x, y)
    raise "[#{x}, #{y}] is empty" if @state[x][y].piece.nil?

    @state[x][y].piece = nil
  end

  def within?(x, y)
    x >= 0 && y >= 0 && x < width && y < height
  end

  def [](x)
    @state[x]
  end

  def width
    @state[0].size
  end

  def height
    @state.size
  end

  def to_s
    @state.map { |row| row.map(&:to_s).join(" ") }.join("\n")
  end
end
