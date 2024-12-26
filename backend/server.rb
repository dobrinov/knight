require 'sinatra'
require_relative './database'
require_relative './lib/main'

before do
  content_type :json
  headers 'Access-Control-Allow-Origin' => '*', 'Access-Control-Allow-Methods' => %w[OPTIONS GET POST]
end

post '/api/users' do
  # Create a user
end

post '/api/guests' do
  request.body.rewind
  body = JSON.parse request.body.read

  Database.exec_params 'INSERT INTO users (name, guest) VALUES ($1, $2) RETURNING *', [body['name'], true]

  # Create guest user and return JWT token
  'thisshouldbeaJWToken'
end

get '/api/lobby' do
  Lobby.load.to_json
end

post '/api/games' do
  map =
    <<~MAP
      X---
      ----
      ----
      ---X
    MAP

  lobby = Lobby.new map: Map.parse(map)

  result =
    Database.exec_params 'INSERT INTO games (state) VALUES ($1) RETURNING *', [lobby.to_json]

  result.values.to_s
end

get '/api/games/:id' do
  result = Database.exec_params 'SELECT * FROM games WHERE id = $1', [params[:id]]

  state = JSON.parse result[0]['state']
  map = state['map']
  players = state['players']

  result[0].to_json
end

post '/api/games/:id/join' do
  # Get user from JWT token

  result = Database.exec_params 'SELECT * FROM games WHERE id = $1', [params[:id]]
end
