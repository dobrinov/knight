require 'spec_helper'
require_relative '../lib/main'

describe Board do
  it 'parses board size' do
    board =
      Board.parse <<~BOARD
        ----- ----- -----
        ----- ----- -----
      BOARD

    board.width.should eq 3
    board.height.should eq 2
  end

  it 'parses an empty block' do
    board =
      Board.parse <<~BOARD
        ----- -----
        ----- -----
      BOARD

    block = board[0][0]

    block.should be_a Block
    block.player.should be_nil
    block.piece.should be_nil
  end

  it 'parses a block with a piece' do
    board =
      Board.parse <<~BOARD
        1P123 -----
        ----- -----
      BOARD

    block = board[0][0]
    piece = block.piece

    block.player.should eq 1
    piece.should be_a Pawn
    piece.attack.should eq 1
    piece.defense.should eq 2
    piece.health.should eq 3
  end
end
