class Game
  attr_reader :players, :board

  def initialize(min_players:, max_players:, first_player_selection: :fifo)
    @min_players = min_players
    @max_players = max_players
    @first_player_selection = first_player_selection
    @board = nil
    @current_player_index = nil
    @players = []
    @moves = []
    @started_at = nil
  end

  def started?
    !@started_at.nil?
  end

  def add_player(name)
    raise "Cannot add a player to a started game" if started?
    raise "Cannot add more than #{@max_players} players" if @players.size >= @max_players

    @players << name
    players.size - 1
  end

  def start
    @started_at = Time.now
    @current_player_index =
      case @first_player_selection
      when :fifo
        0
      when :random
        rand @players.count
      else
        raise "Unknown first player selection strategy - #{@first_player_selection}"
      end

    @board =
      Board.parse <<~BOARD
        -0K102 ------ ------ ------
        ------ ------ ------ ------
        ------ ------ ------ ------
        ------ ------ ------ -1K102
      BOARD
  end

  def move_piece(from:, to:)
    fx, fy = from
    tx, ty = to

    block = @board[fx][fy]
    piece = block.piece
    raise "No piece at [#{fx}, #{fy}]" if piece.nil?

    available_paths =
      piece.move_pattern.map do |step|
        x, y = step
        path = []

        while \
          (
            piece.move_pattern_repetition_limit.nil? ||
            path.size < piece.move_pattern_repetition_limit
          ) &&
          @board.within?(fx + x, fy + y) &&
          (
            @board[fx + x][fy + y].piece.nil? ||
            (
              @board[fx + x][fy + y].piece &&
              @board[fx + x][fy + y].player != @current_player_index
            )
          ) do

          path << [fx + x, fy + y]

          x += step[0]
          y += step[1]
        end

        path
      end.filter { _1.size > 0 }

    path = available_paths.find { _1.include? to }

    raise "Cannot move to #{to}" unless path

    destination = @board[tx][ty]

    if destination.piece.nil?
      @board.move_piece from, to, piece, @current_player_index
    else
      enemy_piece = destination.piece
      enemy_piece.receive_attack piece.attack

      if enemy_piece.alive?
        piece.receive_attack enemy_piece.attack
        @board.remove_piece *from
        if piece.alive?
          previous_path_step = path.size > 1 ? path[path.index(to) - 1] : from
          @board.place_piece *previous_path_step, piece, @current_player_index
        end
      else
        @board.remove_piece *to
        @board.move_piece from, to, piece, @current_player_index
      end
    end
  end

  def inspect
    @board.to_s
  end
end
