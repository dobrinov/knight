require 'sinatra'
require_relative './database'
require_relative './lib/main'

DB = Database.connection_pool

set :json_content_type, :js

post '/api/users' do
  # Create a user
end

post '/api/guests' do
  request.body.rewind
  body = JSON.parse request.body.read

  # Create guest user and return JWT token
  DB.with do |psql|
    psql.exec_params('INSERT INTO users (name, guest) VALUES ($1, $2) RETURNING *', [params['name'], true])
  end
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
    DB.with do |psql|
      psql.exec_params('INSERT INTO games (state) VALUES ($1) RETURNING *', [lobby.to_json])
    end

  result.values.to_s
end

get '/api/games/:id' do
  result =
    DB.with do |psql|
      psql.exec_params('SELECT * FROM games WHERE id = $1', [params[:id]])
    end

  state = JSON.parse result[0]['state']
  map = state['map']
  players = state['players']

  result[0].to_json
end

post '/api/games/:id/join' do
  # Get user from JWT token

  result =
    DB.with do |psql|
      psql.exec_params('SELECT * FROM games WHERE id = $1', [params[:id]])
    end
end
