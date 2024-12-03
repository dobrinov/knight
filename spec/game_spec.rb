require 'spec_helper'
require_relative '../lib/main'

describe 'Game' do
  it 'plays' do
    map =
      <<~MAP
        X---
        ----
        ----
        ---X
      MAP

    lobby = Lobby.new map: Map.parse(map)
    victor = lobby.add_player 'Victor'
    alexander = lobby.add_player 'Alexander'
    game = lobby.start_game

    game.move_piece from: [0, 0], to: [1, 1]
    game.move_piece from: [1, 1], to: [2, 2]
    game.move_piece from: [2, 2], to: [3, 3]

    # p game
  end

  describe 'ending turns' do
    it 'switches between players' do
      map =
        <<~MAP
          X-
          -X
        MAP

      lobby = Lobby.new map: Map.parse(map)
      victor = lobby.add_player 'Victor'
      alexander = lobby.add_player 'Alexander'
      game = lobby.start_game

      game.current_player.should eq victor
      game.end_turn
      game.current_player.should eq alexander
      game.end_turn
      game.current_player.should eq victor
    end

    it 'distributes resources from occupied blocks' do
      board =
        <<~BOARD
          -0K101 W0---- I1----
          W0---- G0---- S1----
          ------ W1---- -1K101
        BOARD

      victor = Player.new id: 0, name: 'Victor', wood: 0, stone: 0, iron: 0, gold: 0
      alexander = Player.new id: 1, name: 'Alexander', wood: 0, stone: 0, iron: 0, gold: 0

      game = Game.new board: Board.parse(board), players: [victor, alexander]

      game.end_turn

      victor.wood.should eq 2
      victor.stone.should eq 0
      victor.iron.should eq 0
      victor.gold.should eq 1

      alexander.wood.should eq 1
      alexander.stone.should eq 1
      alexander.iron.should eq 1
      alexander.gold.should eq 0
    end
  end
end
