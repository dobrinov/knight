require 'spec_helper'
require_relative '../lib/main'

describe Block do
  it 'raises error if invalid resource is used' do
    expect { Block.new x: 0, y: 0, resource: :coal }.to raise_error ArgumentError, "Invalid resource: coal"
  end
end
