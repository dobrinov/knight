class Lobby
  attr_reader :min_players

  class << self
    def create(map)
      new map: Map.parse(map)
    end

    def load(first: 10)
      result = Database.exec_params 'SELECT * FROM games WHERE started_at IS NULL LIMIT $1', [first]

      result.map do |lobby|
        new id: lobby['id'], map: Map.parse(lobby['state']['map'])
      end
    end
  end

  def initialize(map:, id: nil, players: [])
    @id = id
    @map = map
    @players = players
  end

  def add_player(name)
    raise "Cannot add more than #{@map.max_players} players" if @players.size >= @map.max_players

    player = Player.new id: @players.size, name: name
    @players << player
    player
  end

  def start_game
    raise 'Not enough players' if @players.size < @min_players

    Game.new board: @map.to_board(@players.size), players: @players, current_player_index: 0
  end

  def max_players
    @map.max_players
  end

  def to_json(*_args)
    {
      id: @id,
      map: @map.to_s,
      min_players: @min_players,
      max_players: max_players,
      players: @players.map(&:to_json)
    }.to_json
  end
end
