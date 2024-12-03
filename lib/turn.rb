class Turn
  def initialize(player:)
    @player = player
    @completed_at = nil
  end

  def completed?
    !!@completed_at
  end

  def complete
    @completed_at = Time.now
  end
end
