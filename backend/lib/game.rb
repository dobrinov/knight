class Game
  attr_reader :players, :board

  def initialize(board:, players:, current_player_index: 0)
    @board = board
    @players = players
    @current_player_index = current_player_index
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
              @board[fx + x][fy + y].player_id != @current_player_index
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

  def current_player
    @players[@current_player_index]
  end

  def end_turn
    @board.owned_blocks.each do |block|
      @players[block.player_id].give_resource resource: block.resource, amount: 1 if block.resource
    end

    @current_player_index = (@current_player_index + 1) % @players.count
  end

  def inspect
    @board.to_s
  end
end
