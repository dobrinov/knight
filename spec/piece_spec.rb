require 'spec_helper'
require_relative '../lib/main/'

describe Piece do
  it 'parses a Pawn' do
    piece = Piece.parse 'P111'
    piece.should be_a Pawn
  end

  it 'parses a Rook' do
    piece = Piece.parse 'R111'
    piece.should be_a Rook
  end

  it 'parses a Knight' do
    piece = Piece.parse 'N111'
    piece.should be_a Knight
  end

  it 'parses a Bishop' do
    piece = Piece.parse 'B111'
    piece.should be_a Bishop
  end

  it 'parses a Queen' do
    piece = Piece.parse 'Q111'
    piece.should be_a Queen
  end

  it 'raises error on invalid piece type' do
    expect { Piece.parse 'X111' }.to raise_error 'Unknown piece type: X'
  end

  it 'parses attack' do
    piece = Piece.parse 'P211'
    piece.attack.should == 2
  end

  it 'parses defense' do
    piece = Piece.parse 'P121'
    piece.defense.should == 2
  end

  it 'parses health' do
    piece = Piece.parse 'P113'
    piece.health.should == 3
  end
end
