class Game
  attr_reader :players, :board

  Player = Data.define :name, :wood, :stone, :iron, :gold

  class << self
    def load(board:, current_player_index:)
    end
  end

  def initialize(map:, min_players:, max_players:, first_player_selection: :fifo)
    @min_players = min_players
    @max_players = max_players
    @first_player_selection = first_player_selection
    @map = Map.parse map
    @board = nil
    @current_player_index = nil
    @players = []
    @turns = []
    @started_at = nil
  end

  def started?
    !@started_at.nil?
  end

  def add_player(name)
    raise "Cannot add a player to a started game" if started?
    raise "Cannot add more than #{@max_players} players" if @players.size >= @max_players

    player = Player.new name: name, wood: 0, stone: 0, iron: 0, gold: 0
    @players << player
    player
  end

  def start
    @started_at = Time.now
    @board = @map.to_board @players.size

    start_next_turn
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

  def current_turn
    turn = @turns.last

    return turn unless turn.completed?
  end

  def current_player
    @players[@current_player_index]
  end

  def end_turn
    current_turn = @turns.last
    raise "Cannot end an ended turn" if current_turn.completed?

    # Get all blocks controlled by the current player and give them resources

    current_turn.complete
    start_next_turn
  end

  def inspect
    @board.to_s
  end

  private

  def start_next_turn
    raise "Cannot start a new turn without starting the game" unless started?
    raise "Cannot start a new turn if no players are available" if @players.empty?
    raise "Cannot start a new turn if the last one is not finished yet" if @turns.last && !@turns.last.completed?

    @current_player_index =
      if @current_player_index.nil? && @first_player_selection == :fifo
        0
      elsif @current_player_index.nil? && @first_player_selection == :random
        rand @players.count
      elsif @current_player_index.nil?
        raise "Unknown first player selection strategy - #{@first_player_selection}"
      else
        (@current_player_index + 1) % @players.count
      end

    @turns << Turn.new(player: @current_player_index)
  end
end
