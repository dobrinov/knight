class Lobby
  class << self
    def create(map)
      new map: Map.parse(map)
    end
  end

  def initialize(map:, min_players: 2, players: [])
    @map = map
    @min_players = min_players
    @players = players
  end

  def add_player(name)
    raise "Cannot add more than #{@map.max_players} players" if @players.size >= @map.max_players

    player = Player.new id: @players.size, name: name
    @players << player
    player
  end

  def start_game
    raise "Not enough players" if @players.size < @min_players

    Game.new board: @map.to_board(@players.size), players: @players, current_player_index: 0
  end

  def to_json
    {
      map: @map.to_s,
      players: @players.map(&:to_json)
    }.to_json
  end
end
